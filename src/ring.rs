//! The receive and transmit descriptor rings, polled, one 2 KiB
//! driver-owned buffer per descriptor, legacy (16-byte) descriptors.
//!
//! Formats (docs/hardware.md). Receive: the 64-bit buffer address, then
//! length (16 bits, CRC stripped by RCTL.SECRC), checksum, status (DD 0,
//! EOP 1), errors, special. Transmit: the 64-bit buffer address, then
//! length, CSO, command (EOP 0, IFCS 1, RS 3), status (DD 0), CSS, special.
//!
//! Ownership is by head and tail registers, not an OWN bit: the hardware
//! owns receive descriptors from RDH up to (not including) RDT and transmit
//! descriptors from TDH up to TDT. One receive descriptor is always held
//! back (RDT starts at count - 1), so a full ring never looks empty; a
//! taken one is given back by writing its index to RDT. Transmit keeps one
//! free, so TDT never catches up with TDH. The driver learns that the
//! hardware is done with a descriptor from its DD bit.
//!
//! Region layout (`DMA_BYTES`, one PciIo common buffer): the receive ring
//! at 0 (32 x 16 B), the transmit ring at 1024 (32 x 16 B; both 128-byte
//! aligned, the lengths multiples of 128), then from 4096 the 32 receive
//! buffers and the 32 transmit buffers. Frames are copied in and out, so no
//! caller memory is mapped for DMA. Every access to the region is volatile
//! and little-endian.

use super::barrier::{dma_rmb, dma_wmb, mmio_wmb};
use super::{Bus, Error, Kind, Link, Rings, R, RDT, TDT};
use core::ptr;

pub const RX_COUNT: usize = 32;
pub const TX_COUNT: usize = 32;
pub const BUF_SIZE: usize = 2048;
const DESC: usize = 16;
pub const RX_RING: usize = 0;
pub const TX_RING: usize = 1024;
pub const RX_BUFS: usize = 4096;
pub const TX_BUFS: usize = RX_BUFS + RX_COUNT * BUF_SIZE;
pub const DMA_BYTES: usize = TX_BUFS + TX_COUNT * BUF_SIZE;
pub const DMA_PAGES: usize = DMA_BYTES.div_ceil(4096);

/// Descriptor offsets of the length word and the status word.
pub const LEN: usize = 8;
pub const STA: usize = 12;
/// Status: descriptor done; receive: end of packet.
pub const DD: u32 = 0x01;
pub const RX_EOP: u32 = 0x02;
/// Receive errors (status word bits 8-15): CRC, symbol, sequence, carrier
/// extension, RX data error. TCP/IP checksum errors are not: no offload.
pub const RX_ERRORS: u32 = 0x97 << 8;
/// Transmit command (length word bits 24-31): end of packet, insert CRC,
/// report status.
pub const TX_EOP: u32 = 0x01 << 24;
pub const TX_IFCS: u32 = 0x02 << 24;
pub const TX_RS: u32 = 0x08 << 24;

/// Largest frame without CRC: 1514 plus a 4-byte VLAN tag.
pub const MAX_FRAME: usize = 1518;
/// Ethernet header: the smallest frame the driver will send.
pub const MIN_FRAME: usize = 14;
/// Short frames are padded to this (64 bytes on the wire with the CRC the
/// MAC appends).
pub const PAD_TO: usize = 60;

/// The DMA region: `DMA_BYTES` at `host`, which the device reaches at `device`.
#[derive(Clone, Copy, Debug)]
pub struct Dma {
    pub host: *mut u8,
    pub device: u64,
}

/// A frame's Ethernet header fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub destination: [u8; 6],
    pub source: [u8; 6],
    pub ethertype: u16,
}

pub fn frame(f: &[u8]) -> Frame {
    let mut d = [0u8; 6];
    let mut s = [0u8; 6];
    d.copy_from_slice(&f[0..6]);
    s.copy_from_slice(&f[6..12]);
    Frame { destination: d, source: s, ethertype: u16::from_be_bytes([f[12], f[13]]) }
}

pub struct Net {
    dma: Dma,
    pub kind: Kind,
    pub mac: [u8; 6],
    /// The receive descriptor to look at next.
    rx_next: usize,
    /// Transmit descriptors handed to the hardware and not yet seen back:
    /// `tx_cons` up to (not including) `tx_prod`, counted modulo 2^usize.
    tx_prod: usize,
    tx_cons: usize,
    pub running: bool,
    /// Receive descriptors that came back with an error, a frame split over
    /// several buffers or a runt, since `start`.
    pub rx_errors: usize,
}

impl Net {
    /// # Safety
    /// `dma.host` is `DMA_BYTES` of memory the device reaches at
    /// `dma.device`, used by nothing else while this `Net` lives.
    pub unsafe fn new(dma: Dma, kind: Kind, mac: [u8; 6]) -> Net {
        Net { dma, kind, mac, rx_next: 0, tx_prod: 0, tx_cons: 0, running: false, rx_errors: 0 }
    }

    pub fn rings(&self) -> Rings {
        Rings {
            rx: self.dma.device + RX_RING as u64,
            rx_count: RX_COUNT as u32,
            tx: self.dma.device + TX_RING as u64,
            tx_count: TX_COUNT as u32,
        }
    }

    fn r32(&self, off: usize) -> u32 {
        // SAFETY: `off` + 4 is inside the region (callers index the rings).
        u32::from_le(unsafe { ptr::read_volatile(self.dma.host.add(off) as *const u32) })
    }

    fn w32(&self, off: usize, v: u32) {
        // SAFETY: as `r32`.
        unsafe { ptr::write_volatile(self.dma.host.add(off) as *mut u32, v.to_le()) }
    }

    fn w64(&self, off: usize, v: u64) {
        self.w32(off, v as u32);
        self.w32(off + 4, (v >> 32) as u32);
    }

    fn rx_desc(i: usize) -> usize { RX_RING + i * DESC }
    fn tx_desc(i: usize) -> usize { TX_RING + i * DESC }

    /// Make receive descriptor `i` ready for the hardware: its buffer, and
    /// the status cleared.
    fn post_rx(&self, i: usize) {
        let d = Self::rx_desc(i);
        self.w64(d, self.dma.device + (RX_BUFS + i * BUF_SIZE) as u64);
        self.w32(d + LEN, 0);
        self.w32(d + STA, 0);
    }

    /// The whole bring-up: MAC reset, every receive buffer posted,
    /// `mac_init` with these rings, RX and TX on. On failure the NIC is
    /// quiesced again and nothing runs.
    pub fn start<B: Bus>(&mut self, b: &mut B, promiscuous: bool) -> R<(), B::Error> {
        self.running = false;
        let r = self.bring_up(b, promiscuous);
        if r.is_err() {
            let _ = super::quiesce(b, self.kind);
        }
        r
    }

    fn bring_up<B: Bus>(&mut self, b: &mut B, promiscuous: bool) -> R<(), B::Error> {
        super::disable(b)?;
        super::reset(b, self.kind)?;
        // The MAC is reset: the rings are ours to rewrite.
        // SAFETY: the two rings are inside the region, below RX_BUFS.
        unsafe { ptr::write_bytes(self.dma.host, 0, RX_BUFS) };
        for i in 0..TX_COUNT {
            self.w64(Self::tx_desc(i), self.dma.device + (TX_BUFS + i * BUF_SIZE) as u64);
            // Done: free for the driver.
            self.w32(Self::tx_desc(i) + STA, DD);
        }
        for i in 0..RX_COUNT { self.post_rx(i); }
        self.rx_next = 0;
        self.tx_prod = 0;
        self.tx_cons = 0;
        self.rx_errors = 0;
        dma_wmb();
        mmio_wmb();
        super::mac_init(b, self.kind, self.mac, self.rings(), promiscuous)?;
        super::enable(b)?;
        self.running = true;
        Ok(())
    }

    /// Quiesce the NIC. Ok: it does no DMA any more.
    pub fn stop<B: Bus>(&mut self, b: &mut B) -> R<(), B::Error> {
        self.running = false;
        super::quiesce(b, self.kind)
    }

    pub fn link<B: Bus>(&self, b: &mut B) -> R<Link, B::Error> { super::link(b) }

    pub fn set_promiscuous<B: Bus>(&self, b: &mut B, on: bool) -> R<(), B::Error> {
        if self.running { super::set_filter(b, on)?; }
        Ok(())
    }

    /// Transmit descriptors the hardware still has.
    pub fn tx_pending(&self) -> usize { self.tx_prod.wrapping_sub(self.tx_cons) }

    /// Retire the transmit descriptors the hardware has written back.
    pub fn reclaim(&mut self) {
        while self.tx_pending() > 0 {
            let i = self.tx_cons % TX_COUNT;
            if self.r32(Self::tx_desc(i) + STA) & DD == 0 { break; }
            self.tx_cons = self.tx_cons.wrapping_add(1);
        }
        dma_rmb();
    }

    /// Copy `f` into the next transmit buffer and hand it to the hardware.
    /// Ok(false) when the ring is full.
    pub fn transmit<B: Bus>(&mut self, b: &mut B, f: &[u8]) -> R<bool, B::Error> {
        if !(MIN_FRAME..=MAX_FRAME).contains(&f.len()) { return Err(Error::Frame { len: f.len() }); }
        self.reclaim();
        if self.tx_pending() == TX_COUNT - 1 { return Ok(false); }
        let i = self.tx_prod % TX_COUNT;
        let buf = TX_BUFS + i * BUF_SIZE;
        let len = f.len().max(PAD_TO);
        // SAFETY: a 2 KiB buffer inside the region that the hardware does
        // not have (its descriptor is retired); `len` <= 1518.
        unsafe {
            let dst = self.dma.host.add(buf);
            for (k, byte) in f.iter().enumerate() { ptr::write_volatile(dst.add(k), *byte); }
            for k in f.len()..len { ptr::write_volatile(dst.add(k), 0); }
        }
        let d = Self::tx_desc(i);
        self.w64(d, self.dma.device + buf as u64);
        self.w32(d + STA, 0);
        self.w32(d + LEN, TX_EOP | TX_IFCS | TX_RS | len as u32);
        self.tx_prod = self.tx_prod.wrapping_add(1);
        mmio_wmb();
        b.w32(TDT, (self.tx_prod % TX_COUNT) as u32)?;
        Ok(true)
    }

    /// The next good received frame: its length and destination, without
    /// taking it. Descriptors with errors, split frames and runts are given
    /// back to the hardware on the way.
    pub fn peek<B: Bus>(&mut self, b: &mut B) -> R<Option<(usize, [u8; 6])>, B::Error> {
        if !self.running { return Ok(None); }
        loop {
            let d = Self::rx_desc(self.rx_next);
            let s = self.r32(d + STA);
            if s & DD == 0 { return Ok(None); }
            dma_rmb();
            let len = (self.r32(d + LEN) & 0xffff) as usize;
            if s & RX_ERRORS != 0 || s & RX_EOP == 0 || !(MIN_FRAME..=BUF_SIZE).contains(&len) {
                self.rx_errors += 1;
                self.skip(b)?;
                continue;
            }
            let mut dst = [0u8; 6];
            let buf = RX_BUFS + self.rx_next * BUF_SIZE;
            for (k, byte) in dst.iter_mut().enumerate() {
                // SAFETY: inside the received buffer, which the hardware
                // handed back (DD set, ordered by dma_rmb).
                *byte = unsafe { ptr::read_volatile(self.dma.host.add(buf + k)) };
            }
            return Ok(Some((len, dst)));
        }
    }

    /// Give the current receive descriptor back without reading it.
    pub fn skip<B: Bus>(&mut self, b: &mut B) -> R<(), B::Error> {
        let i = self.rx_next;
        self.post_rx(i);
        self.rx_next = (i + 1) % RX_COUNT;
        mmio_wmb();
        b.w32(RDT, i as u32)?;
        Ok(())
    }

    /// Take the next good frame into `out`: Ok(Some(len)), Ok(None) when
    /// nothing is waiting. A frame longer than `out` stays queued
    /// (`Error::Frame` with its length).
    pub fn receive<B: Bus>(&mut self, b: &mut B, out: &mut [u8]) -> R<Option<usize>, B::Error> {
        let Some((len, _)) = self.peek(b)? else { return Ok(None) };
        if len > out.len() { return Err(Error::Frame { len }); }
        let buf = RX_BUFS + self.rx_next * BUF_SIZE;
        for (k, byte) in out[..len].iter_mut().enumerate() {
            // SAFETY: as in `peek`.
            *byte = unsafe { ptr::read_volatile(self.dma.host.add(buf + k)) };
        }
        self.skip(b)?;
        Ok(Some(len))
    }
}

/// The frame `check` sends: broadcast, ethertype 0x88b5 (IEEE local
/// experimental), padded to 60 bytes.
pub fn check_frame(mac: [u8; 6]) -> [u8; 60] {
    let mut f = [0u8; 60];
    f[0..6].copy_from_slice(&[0xff; 6]);
    f[6..12].copy_from_slice(&mac);
    f[12..14].copy_from_slice(&0x88b5u16.to_be_bytes());
    let text = b"stormnic-e1000e DMA check";
    f[14..14 + text.len()].copy_from_slice(text);
    f
}

/// Send the check frame and wait up to 100 ms for the hardware to write its
/// descriptor back (DD): it fetched the descriptor and the buffer and wrote
/// the status, so DMA works both ways. Ok(Some(ms)) when it did.
pub fn check<B: Bus>(b: &mut B, net: &mut Net) -> R<Option<usize>, B::Error> {
    if !net.transmit(b, &check_frame(net.mac))? { return Ok(None); }
    for ms in 0..=100 {
        net.reclaim();
        if net.tx_pending() == 0 { return Ok(Some(ms)); }
        b.delay_us(1000);
    }
    Ok(None)
}
