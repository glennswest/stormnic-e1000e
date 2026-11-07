//! A simulated e1000e that DMAs, shared by the host tests: a register file
//! with the registers that act (CTRL's reset, STATUS from a modelled link,
//! EERD from an EEPROM, MDIC to a PHY, the PHY semaphore, ICR's
//! clear-on-read, the TDT doorbell), and legacy descriptor rings walked by
//! head and tail the way docs/hardware.md describes.
use crate::e1000e::ring::{self, Dma, Net, DMA_BYTES};
use crate::e1000e::{self as nic, Bus, Kind};
use std::collections::HashMap;

pub const MAC: [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
/// Where the simulated IOMMU puts the region.
pub const DEVICE_BASE: u64 = 0x1_2345_0000;
/// Autonegotiation time after a restart.
pub const AN_US: u64 = 2_000_000;
pub const PHY_ID: u32 = 0x0141_0cb0;

pub struct Dev {
    pub kind: Kind,
    base: *mut u8,
    pub regs: HashMap<u32, u32>,
    pub eeprom: [u16; 64],
    pub phy: HashMap<u32, u16>,
    /// Microseconds of Stall so far.
    pub now: u64,
    pub cable: bool,
    /// Autonegotiation completes at this time.
    an_done_at: u64,
    pub an_restarts: usize,
    pub resets: usize,
    /// CTRL.RST never clears.
    pub reset_stuck: bool,
    /// MDIC never reports ready.
    pub mdic_stuck: bool,
    /// No PHY at the MDIO address: MDIC reports an error.
    pub phy_absent: bool,
    /// The management firmware holds the PHY semaphore.
    pub fw_semaphore: bool,
    /// The NVM leaves receive address 0 empty at reset.
    pub ral_blank: bool,
    /// TDT writes are ignored (a hung transmitter).
    pub tx_stalled: bool,
    pub wire: Vec<Vec<u8>>,
    pub loopback: bool,
    /// Frames lost for want of a descriptor or with RX off.
    pub dropped: usize,
    /// Frames the address filter refused.
    pub filtered: usize,
    /// Every register write as (offset, value).
    pub ops: Vec<(u32, u32)>,
}

impl Dev {
    pub fn new(kind: Kind) -> Dev {
        let layout = std::alloc::Layout::from_size_align(DMA_BYTES, 4096).unwrap();
        // SAFETY: a non-zero layout; the memory is leaked for the test.
        let base = unsafe { std::alloc::alloc_zeroed(layout) };
        let mut eeprom = [0u16; 64];
        for w in 0..3 { eeprom[w] = u16::from_le_bytes([MAC[2 * w], MAC[2 * w + 1]]); }
        let mut d = Dev {
            kind, base, regs: HashMap::new(), eeprom, phy: HashMap::new(), now: 0, cable: true,
            an_done_at: 0, an_restarts: 0, resets: 0, reset_stuck: false, mdic_stuck: false,
            phy_absent: false, fw_semaphore: false, ral_blank: false, tx_stalled: false,
            wire: Vec::new(), loopback: false, dropped: 0, filtered: 0, ops: Vec::new(),
        };
        d.phy.insert(nic::MII_PHYID1, (PHY_ID >> 16) as u16);
        d.phy.insert(nic::MII_PHYID2, PHY_ID as u16);
        d.phy.insert(nic::MII_BMCR, 0x1140);
        d.phy.insert(nic::MII_BMSR, 0x796d);
        d.power_on();
        // What a firmware driver typically leaves: link up, flow control on.
        d.put(nic::CTRL, nic::CTRL_SLU | nic::CTRL_ASDE | nic::CTRL_FD | nic::CTRL_RFCE | nic::CTRL_TFCE);
        d.put(nic::FCTTV, 0x0680);
        d.put(nic::FCRTH, 0x8000_3000);
        d
    }

    /// Register state after power-on or a MAC reset.
    fn power_on(&mut self) {
        let keep_status = self.get(nic::STATUS) & nic::STATUS_LAN_INIT_DONE;
        let phy = std::mem::take(&mut self.phy);
        self.regs.clear();
        self.phy = phy;
        self.put(nic::CTRL, nic::CTRL_FD | nic::CTRL_ASDE);
        if !self.ral_blank {
            self.put(nic::RAL0, u32::from_le_bytes([MAC[0], MAC[1], MAC[2], MAC[3]]));
            self.put(nic::RAH0, u32::from(MAC[4]) | (u32::from(MAC[5]) << 8) | nic::RAH_AV);
        }
        self.put(nic::TXDCTL, 0x0001_0000);
        if self.kind == Kind::Pch { self.put(nic::STATUS, nic::STATUS_LAN_INIT_DONE | keep_status); }
    }

    pub fn dma(&self) -> Dma { Dma { host: self.base, device: DEVICE_BASE } }

    /// A Net over this device's region.
    pub fn net(&self) -> Net {
        // SAFETY: the region is DMA_BYTES, leaked, used only by this Net and
        // the simulated DMA below.
        unsafe { Net::new(self.dma(), self.kind, MAC) }
    }

    pub fn get(&self, r: u32) -> u32 { self.regs.get(&r).copied().unwrap_or(0) }
    pub fn put(&mut self, r: u32, v: u32) { self.regs.insert(r, v); }

    fn host(&self, device: u64, len: usize) -> *mut u8 {
        let off = device.checked_sub(DEVICE_BASE).expect("DMA below the region") as usize;
        assert!(off + len <= DMA_BYTES, "DMA past the region: {off:#x}+{len}");
        // SAFETY: inside the region (checked).
        unsafe { self.base.add(off) }
    }
    pub fn mem32(&self, device: u64) -> u32 {
        // SAFETY: inside the region.
        u32::from_le(unsafe { std::ptr::read_volatile(self.host(device, 4) as *const u32) })
    }
    fn set_mem32(&self, device: u64, v: u32) {
        // SAFETY: inside the region.
        unsafe { std::ptr::write_volatile(self.host(device, 4) as *mut u32, v.to_le()) }
    }
    fn mem64(&self, device: u64) -> u64 { u64::from(self.mem32(device)) | u64::from(self.mem32(device + 4)) << 32 }

    pub fn rx_enabled(&self) -> bool { self.get(nic::RCTL) & nic::RCTL_EN != 0 }
    pub fn tx_enabled(&self) -> bool { self.get(nic::TCTL) & nic::TCTL_EN != 0 }
    fn rx_ring(&self) -> u64 { u64::from(self.get(nic::RDBAL)) | u64::from(self.get(nic::RDBAH)) << 32 }
    fn tx_ring(&self) -> u64 { u64::from(self.get(nic::TDBAL)) | u64::from(self.get(nic::TDBAH)) << 32 }
    fn rx_count(&self) -> u32 { self.get(nic::RDLEN) / 16 }
    fn tx_count(&self) -> u32 { self.get(nic::TDLEN) / 16 }

    pub fn link_up(&self) -> bool {
        let bmcr = self.phy.get(&nic::MII_BMCR).copied().unwrap_or(0);
        self.cable && self.get(nic::CTRL) & nic::CTRL_SLU != 0 && self.now >= self.an_done_at
            && bmcr & nic::BMCR_AUTOEN != 0 && bmcr & nic::BMCR_PDOWN == 0
    }

    fn status(&self) -> u32 {
        let mut s = self.get(nic::STATUS) & nic::STATUS_LAN_INIT_DONE;
        if self.link_up() { s |= nic::STATUS_LU | nic::STATUS_FD | (2 << nic::STATUS_SPEED_SHIFT); }
        if self.get(nic::CTRL) & nic::CTRL_GIO_MASTER_DISABLE == 0 { s |= nic::STATUS_GIO_MASTER_EN; }
        s
    }

    fn mdic(&mut self, v: u32) {
        let reg = (v >> nic::MDIC_REG_SHIFT) & 0x1f;
        let phy = (v >> nic::MDIC_PHY_SHIFT) & 0x1f;
        if self.mdic_stuck { self.put(nic::MDIC, v & !nic::MDIC_READY); return; }
        if self.phy_absent || phy != nic::PHY_ADDR { self.put(nic::MDIC, v | nic::MDIC_READY | nic::MDIC_ERROR); return; }
        if v & nic::MDIC_OP_READ == nic::MDIC_OP_READ {
            let data = self.phy.get(&reg).copied().unwrap_or(0);
            self.put(nic::MDIC, (v & 0xffff_0000) | u32::from(data) | nic::MDIC_READY);
        } else {
            let mut data = v as u16;
            if reg == nic::MII_BMCR && data & nic::BMCR_STARTNEG != 0 {
                self.an_restarts += 1;
                self.an_done_at = self.now + AN_US;
                data &= !nic::BMCR_STARTNEG;
            }
            self.phy.insert(reg, data);
            self.put(nic::MDIC, v | nic::MDIC_READY);
        }
    }

    /// The TDT doorbell: send every descriptor from TDH up to TDT.
    fn transmit(&mut self) {
        if !self.tx_enabled() || self.tx_stalled { return; }
        let (ring, count) = (self.tx_ring(), self.tx_count());
        assert!(count > 0, "TDLEN not set");
        let tdt = self.get(nic::TDT);
        assert!(tdt < count, "TDT {tdt} past the ring");
        let mut tdh = self.get(nic::TDH);
        while tdh != tdt {
            let d = ring + 16 * u64::from(tdh);
            let lw = self.mem32(d + ring::LEN as u64);
            assert_eq!(lw & ring::TX_EOP, ring::TX_EOP, "one buffer per frame");
            assert_eq!(lw & ring::TX_IFCS, ring::TX_IFCS, "the MAC adds the CRC");
            let len = (lw & 0xffff) as usize;
            let p = self.host(self.mem64(d), len);
            // SAFETY: inside the region (checked by `host`).
            let f = unsafe { std::slice::from_raw_parts(p, len) }.to_vec();
            if lw & ring::TX_RS != 0 { self.set_mem32(d + ring::STA as u64, ring::DD); }
            tdh = (tdh + 1) % count;
            self.put(nic::TDH, tdh);
            if self.loopback { self.deliver(&f); }
            self.wire.push(f);
        }
    }

    fn accepts(&self, f: &[u8]) -> bool {
        let rctl = self.get(nic::RCTL);
        let ral = self.get(nic::RAL0).to_le_bytes();
        let rah = self.get(nic::RAH0);
        let station = [ral[0], ral[1], ral[2], ral[3], rah as u8, (rah >> 8) as u8];
        let dst = &f[0..6];
        rctl & nic::RCTL_UPE != 0
            || (rah & nic::RAH_AV != 0 && dst == station)
            || (dst == [0xff; 6] && rctl & nic::RCTL_BAM != 0)
            || (dst[0] & 1 != 0 && dst != [0xff; 6] && rctl & nic::RCTL_MPE != 0)
    }

    /// The next receive descriptor the hardware may fill, or None (ring
    /// full: RDH has caught up with RDT).
    fn rx_slot(&mut self) -> Option<u64> {
        if !self.rx_enabled() { return None; }
        let count = self.rx_count();
        let (rdh, rdt) = (self.get(nic::RDH), self.get(nic::RDT));
        assert!(rdt < count, "RDT {rdt} past the ring");
        if rdh == rdt { return None; }
        self.put(nic::RDH, (rdh + 1) % count);
        Some(self.rx_ring() + 16 * u64::from(rdh))
    }

    /// A frame from the network (the CRC stripped, as RCTL.SECRC asks).
    pub fn deliver(&mut self, f: &[u8]) {
        if !self.rx_enabled() { self.dropped += 1; return; }
        if !self.accepts(f) { self.filtered += 1; return; }
        assert_ne!(self.get(nic::RCTL) & nic::RCTL_SECRC, 0, "CRC stripping is on");
        let Some(d) = self.rx_slot() else { self.dropped += 1; return };
        assert!(f.len() <= ring::BUF_SIZE);
        let p = self.host(self.mem64(d), f.len());
        // SAFETY: inside the region.
        unsafe { std::ptr::copy_nonoverlapping(f.as_ptr(), p, f.len()) };
        self.set_mem32(d + ring::LEN as u64, f.len() as u32);
        self.set_mem32(d + ring::STA as u64, ring::DD | ring::RX_EOP);
    }

    /// Hand back the next receive descriptor with this length and status
    /// word (errors in bits 8-15).
    pub fn deliver_raw(&mut self, len: u32, status: u32) {
        let d = self.rx_slot().expect("a free receive descriptor");
        self.set_mem32(d + ring::LEN as u64, len);
        self.set_mem32(d + ring::STA as u64, status);
    }

    fn write(&mut self, offset: u32, v: u32) {
        self.ops.push((offset, v));
        match offset {
            nic::CTRL if v & nic::CTRL_RST != 0 => {
                self.resets += 1;
                self.power_on();
                if self.reset_stuck { self.put(nic::CTRL, v); }
            }
            nic::EERD if v & nic::EERD_START != 0 => {
                let word = ((v >> nic::EERD_ADDR_SHIFT) & 0x3f) as usize;
                self.put(nic::EERD, (u32::from(self.eeprom[word]) << 16) | (v & 0xfffc) | nic::EERD_DONE);
            }
            nic::MDIC => self.mdic(v),
            nic::EXTCNF_CTRL => self.put(offset, if self.fw_semaphore { v & !nic::EXTCNF_SWFLAG } else { v }),
            nic::IMC => { let ims = self.get(nic::IMS); self.put(nic::IMS, ims & !v); }
            nic::TDT => { self.put(offset, v); self.transmit(); }
            nic::STATUS => { let s = self.get(nic::STATUS); self.put(nic::STATUS, s & v & nic::STATUS_LAN_INIT_DONE); }
            _ => self.put(offset, v),
        }
    }

    fn read(&mut self, offset: u32) -> u32 {
        match offset {
            nic::STATUS => self.status(),
            nic::ICR => { let v = self.get(nic::ICR); self.put(nic::ICR, 0); v }
            _ => self.get(offset),
        }
    }

    /// Writes to `offset` so far, in order.
    pub fn writes(&self, offset: u32) -> Vec<u32> {
        self.ops.iter().filter(|o| o.0 == offset).map(|o| o.1).collect()
    }
}

impl Bus for Dev {
    type Error = ();
    fn r32(&mut self, o: u32) -> Result<u32, ()> { Ok(self.read(o)) }
    fn w32(&mut self, o: u32, v: u32) -> Result<(), ()> { self.write(o, v); Ok(()) }
    fn delay_us(&mut self, us: usize) { self.now += us as u64; }
}

/// A controller and a Net brought up on it.
pub fn started(kind: Kind) -> (Dev, Net) {
    let mut d = Dev::new(kind);
    let mut net = d.net();
    net.start(&mut d, false).unwrap();
    (d, net)
}

/// A minimum-size frame to `dst`, its payload tagged with `tag`.
pub fn frame_to(dst: [u8; 6], tag: u8) -> Vec<u8> {
    let mut f = vec![0u8; 60];
    f[0..6].copy_from_slice(&dst);
    f[6..12].copy_from_slice(&[0x02, 0, 0, 0, 0, 9]);
    f[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
    f[14] = tag;
    f
}
