//! The Intel e1000e MAC (82574L and the PCH LAN controllers), independent of
//! UEFI so every sequence can be tested against a simulated controller.
//!
//! Hardware facts (register offsets, bit fields, descriptor formats) are
//! from Intel's public datasheets; docs/hardware.md describes them. iPXE's
//! intel.c and Linux's e1000e were read for behaviour only; the code is
//! written here. What this driver deliberately does not do:
//!
//! - no PHY reset: the link the firmware or power-on brought up stays, and
//!   the PHY is only made to autonegotiate and be powered (`phy_check`);
//! - no interrupts (the SNP polls), one queue, no offloads, no VLAN, legacy
//!   descriptors only;
//! - **no flow control**: CTRL.TFCE/RFCE stay clear and the XON/XOFF
//!   thresholds zero, so the MAC never sends a pause frame (as
//!   stormnic-realtek#2: a ring nobody drains drops frames instead);
//! - none of the PCH errata work-arounds Linux applies on I217/I219 (ULP
//!   exit, descriptor-ring flush before reset, K1 and PLL settings); the PCH
//!   parts are verified in simulation only.
//!
//! The caller reaches the registers through `Bus` (memory BAR 0) and
//! supplies the firmware's Stall. `ring` (the descriptor rings) and `snp`
//! (the SNP state machine) build on this.

#[path = "barrier.rs"]
pub mod barrier;
#[path = "ring.rs"]
pub mod ring;
#[path = "snp_core.rs"]
pub mod snp;

/// PCI vendor of every controller here: Intel.
pub const VENDOR: u16 = 0x8086;

// Registers (memory BAR 0).
pub const CTRL: u32 = 0x0000;
pub const STATUS: u32 = 0x0008;
pub const EERD: u32 = 0x0014;
pub const CTRL_EXT: u32 = 0x0018;
pub const MDIC: u32 = 0x0020;
pub const FCAL: u32 = 0x0028;
pub const FCAH: u32 = 0x002c;
pub const FCT: u32 = 0x0030;
pub const ICR: u32 = 0x00c0;
/// Interrupt mask set: never written (no interrupts); the tests read it.
#[allow(dead_code)]
pub const IMS: u32 = 0x00d0;
pub const IMC: u32 = 0x00d8;
pub const RCTL: u32 = 0x0100;
pub const FCTTV: u32 = 0x0170;
pub const TCTL: u32 = 0x0400;
pub const TIPG: u32 = 0x0410;
pub const EXTCNF_CTRL: u32 = 0x0f00;
pub const FCRTL: u32 = 0x2160;
pub const FCRTH: u32 = 0x2168;
pub const RDBAL: u32 = 0x2800;
pub const RDBAH: u32 = 0x2804;
pub const RDLEN: u32 = 0x2808;
pub const RDH: u32 = 0x2810;
pub const RDT: u32 = 0x2818;
pub const TDBAL: u32 = 0x3800;
pub const TDBAH: u32 = 0x3804;
pub const TDLEN: u32 = 0x3808;
pub const TDH: u32 = 0x3810;
pub const TDT: u32 = 0x3818;
pub const TXDCTL: u32 = 0x3828;
/// Multicast table array: 128 registers.
pub const MTA: u32 = 0x5200;
pub const MTA_COUNT: u32 = 128;
pub const RAL0: u32 = 0x5400;
pub const RAH0: u32 = 0x5404;

// CTRL.
/// Full duplex when forced; the PHY decides here. The tests' simulator sets it.
#[allow(dead_code)]
pub const CTRL_FD: u32 = 1 << 0;
pub const CTRL_GIO_MASTER_DISABLE: u32 = 1 << 2;
pub const CTRL_LRST: u32 = 1 << 3;
pub const CTRL_ASDE: u32 = 1 << 5;
pub const CTRL_SLU: u32 = 1 << 6;
pub const CTRL_ILOS: u32 = 1 << 7;
pub const CTRL_FRCSPD: u32 = 1 << 11;
pub const CTRL_FRCDPLX: u32 = 1 << 12;
pub const CTRL_RST: u32 = 1 << 26;
pub const CTRL_RFCE: u32 = 1 << 27;
pub const CTRL_TFCE: u32 = 1 << 28;
pub const CTRL_VME: u32 = 1 << 30;
pub const CTRL_PHY_RST: u32 = 1 << 31;
// STATUS.
pub const STATUS_FD: u32 = 1 << 0;
pub const STATUS_LU: u32 = 1 << 1;
pub const STATUS_SPEED_SHIFT: u32 = 6;
pub const STATUS_LAN_INIT_DONE: u32 = 1 << 9;
pub const STATUS_GIO_MASTER_EN: u32 = 1 << 19;
// EERD (82574): start, done, word address from bit 2, data in the high half.
pub const EERD_START: u32 = 1 << 0;
pub const EERD_DONE: u32 = 1 << 1;
pub const EERD_ADDR_SHIFT: u32 = 2;
// CTRL_EXT: the driver tells the management firmware it has the NIC.
pub const CTRL_EXT_DRV_LOAD: u32 = 1 << 28;
// MDIC.
pub const MDIC_REG_SHIFT: u32 = 16;
pub const MDIC_PHY_SHIFT: u32 = 21;
pub const MDIC_OP_WRITE: u32 = 1 << 26;
pub const MDIC_OP_READ: u32 = 2 << 26;
pub const MDIC_READY: u32 = 1 << 28;
pub const MDIC_ERROR: u32 = 1 << 30;
/// The PHY's MDIO address on every part here.
pub const PHY_ADDR: u32 = 1;
/// EXTCNF_CTRL: software owns the MDIO / PHY (the semaphore shared with the
/// management firmware).
pub const EXTCNF_SWFLAG: u32 = 1 << 5;
// RCTL.
pub const RCTL_EN: u32 = 1 << 1;
pub const RCTL_UPE: u32 = 1 << 3;
pub const RCTL_MPE: u32 = 1 << 4;
pub const RCTL_BAM: u32 = 1 << 15;
/// Strip the Ethernet CRC: received lengths exclude it.
pub const RCTL_SECRC: u32 = 1 << 26;
// TCTL.
pub const TCTL_EN: u32 = 1 << 1;
pub const TCTL_PSP: u32 = 1 << 3;
pub const TCTL_CT: u32 = 0x0f << 4;
pub const TCTL_COLD: u32 = 0x3f << 12;
pub const TCTL_RTLC: u32 = 1 << 24;
/// IPG for copper: IPGT 8, IPGR1 8, IPGR2 6.
pub const TIPG_COPPER: u32 = 0x0060_2008;
// TXDCTL: thresholds in descriptors (GRAN), write each descriptor back
// (WTHRESH 1), prefetch up to 31; on the 82574 also count descriptors.
pub const TXDCTL_PTHRESH: u32 = 0x3f;
pub const TXDCTL_WTHRESH: u32 = 0x3f << 16;
pub const TXDCTL_GRAN: u32 = 1 << 24;
pub const TXDCTL_COUNT_DESC: u32 = 1 << 22;
pub const RAH_AV: u32 = 1 << 31;

// Standard MII registers of the PHY, and their bits.
pub const MII_BMCR: u32 = 0;
pub const MII_BMSR: u32 = 1;
pub const MII_PHYID1: u32 = 2;
pub const MII_PHYID2: u32 = 3;
pub const BMCR_AUTOEN: u16 = 0x1000;
pub const BMCR_PDOWN: u16 = 0x0800;
pub const BMCR_ISOLATE: u16 = 0x0400;
pub const BMCR_STARTNEG: u16 = 0x0200;

/// How the part keeps its NVM and shares its PHY (ids.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The discrete 82574L: its own EEPROM, read through EERD.
    M82574,
    /// A chipset (PCH) LAN controller: NVM in the platform flash, PHY shared
    /// with the management engine.
    Pch,
}

impl Kind {
    /// Whether this kind has run on hardware or an emulator (the console
    /// says so when it has not).
    pub fn verified(self) -> bool { self == Kind::M82574 }
}

/// The memory BAR's registers and the firmware's Stall.
pub trait Bus {
    type Error: Copy + core::fmt::Debug;
    fn r32(&mut self, offset: u32) -> Result<u32, Self::Error>;
    fn w32(&mut self, offset: u32, value: u32) -> Result<(), Self::Error>;
    fn delay_us(&mut self, micros: usize);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error<E> {
    Io(E),
    /// STATUS reads all ones: the function is gone (or its BAR not decoded).
    Removed,
    /// Neither RAL0/RAH0 nor (82574) the EEPROM holds a unicast address.
    NoMac,
    /// CTRL.RST did not clear within 100 ms of the reset.
    ResetTimeout { ctrl: u32 },
    /// An MDIC access did not complete within 10 ms.
    MdicTimeout { reg: u32 },
    /// The MAC flagged an MDIC access as failed (no PHY at the address).
    MdicFailed { reg: u32 },
    /// A frame outside 14..=1518 bytes to send.
    Frame { len: usize },
}

impl<E> From<E> for Error<E> {
    fn from(e: E) -> Self { Error::Io(e) }
}

pub type R<T, E> = Result<T, Error<E>>;

/// Link state, from STATUS, and what the MAC does about pause (CTRL).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Link {
    pub up: bool,
    pub mbps: u32,
    pub full_duplex: bool,
    /// The MAC honours received pause frames / sends them.
    pub rx_pause: bool,
    pub tx_pause: bool,
}

impl Link {
    pub fn from_regs(status: u32, ctrl: u32) -> Link {
        let up = status & STATUS_LU != 0;
        let mbps = if !up { 0 } else {
            match (status >> STATUS_SPEED_SHIFT) & 3 { 0 => 10, 1 => 100, _ => 1000 }
        };
        Link {
            up,
            mbps,
            full_duplex: status & STATUS_FD != 0,
            rx_pause: ctrl & CTRL_RFCE != 0,
            tx_pause: ctrl & CTRL_TFCE != 0,
        }
    }
}

impl core::fmt::Display for Link {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if !self.up { return f.write_str("down"); }
        write!(f, "up {} Mb/s {}", self.mbps, if self.full_duplex { "full duplex" } else { "half duplex" })?;
        match (self.rx_pause, self.tx_pause) {
            (false, false) => f.write_str(", flow control off"),
            (rx, tx) => write!(f, ", flow control rx {} tx {}", if rx { "on" } else { "off" }, if tx { "on" } else { "off" }),
        }
    }
}

/// Where the station address came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacSource {
    /// Receive address 0, loaded from the NVM by the hardware.
    Ral0,
    /// The 82574's EEPROM words 0-2, through EERD.
    Eeprom,
}

/// What `identify` reads without changing anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub status: u32,
    pub mac: [u8; 6],
    pub mac_from: MacSource,
    pub link: Link,
}

fn unicast(m: [u8; 6]) -> bool {
    m != [0; 6] && m != [0xff; 6] && m[0] & 1 == 0
}

/// STATUS, the station address and the link, read only.
pub fn identify<B: Bus>(b: &mut B, kind: Kind) -> R<Identity, B::Error> {
    let status = b.r32(STATUS)?;
    if status == u32::MAX { return Err(Error::Removed); }
    let (mac, mac_from) = read_mac(b, kind)?;
    let link = link(b)?;
    Ok(Identity { status, mac, mac_from, link })
}

/// The factory address: receive address 0 when it is valid (the hardware
/// loads it from the NVM at power-on and reset), else on the 82574 the
/// EEPROM's first three words.
pub fn read_mac<B: Bus>(b: &mut B, kind: Kind) -> R<([u8; 6], MacSource), B::Error> {
    let lo = b.r32(RAL0)?.to_le_bytes();
    let hi = b.r32(RAH0)?;
    let h = hi.to_le_bytes();
    let ral = [lo[0], lo[1], lo[2], lo[3], h[0], h[1]];
    if hi & RAH_AV != 0 && unicast(ral) { return Ok((ral, MacSource::Ral0)); }
    if kind == Kind::M82574 {
        let mut m = [0u8; 6];
        for w in 0..3 {
            let Some(v) = eerd(b, w)? else { return Err(Error::NoMac) };
            m[2 * w as usize..2 * w as usize + 2].copy_from_slice(&v.to_le_bytes());
        }
        if unicast(m) { return Ok((m, MacSource::Eeprom)); }
    }
    Err(Error::NoMac)
}

/// One EEPROM word through EERD (82574); None if it never reports done.
pub fn eerd<B: Bus>(b: &mut B, word: u32) -> R<Option<u16>, B::Error> {
    b.w32(EERD, (word << EERD_ADDR_SHIFT) | EERD_START)?;
    for _ in 0..1000 {
        let v = b.r32(EERD)?;
        if v & EERD_DONE != 0 { return Ok(Some((v >> 16) as u16)); }
        b.delay_us(5);
    }
    Ok(None)
}

pub fn link<B: Bus>(b: &mut B) -> R<Link, B::Error> {
    let status = b.r32(STATUS)?;
    let ctrl = b.r32(CTRL)?;
    Ok(Link::from_regs(status, ctrl))
}

/// Poll `f` every `step` µs, up to `tries` times; whether it came true.
fn wait<B: Bus>(b: &mut B, tries: usize, step: usize, mut f: impl FnMut(&mut B) -> R<bool, B::Error>) -> R<bool, B::Error> {
    for _ in 0..tries {
        if f(b)? { return Ok(true); }
        b.delay_us(step);
    }
    f(b)
}

fn set32<B: Bus>(b: &mut B, r: u32, bits: u32) -> R<(), B::Error> { let v = b.r32(r)?; Ok(b.w32(r, v | bits)?) }
fn clr32<B: Bus>(b: &mut B, r: u32, bits: u32) -> R<(), B::Error> { let v = b.r32(r)?; Ok(b.w32(r, v & !bits)?) }

/// Take the software semaphore over the PHY (EXTCNF_CTRL.SWFLAG) from the
/// management firmware: set it, and it is ours once it reads back set.
/// Up to 100 ms; false when the firmware never let go (the access goes on,
/// as the datasheet's protocol is advisory for a lone driver).
pub fn semaphore<B: Bus>(b: &mut B) -> R<bool, B::Error> {
    wait(b, 100, 1000, |b| {
        set32(b, EXTCNF_CTRL, EXTCNF_SWFLAG)?;
        Ok(b.r32(EXTCNF_CTRL)? & EXTCNF_SWFLAG != 0)
    })
}

pub fn release_semaphore<B: Bus>(b: &mut B) -> R<(), B::Error> { clr32(b, EXTCNF_CTRL, EXTCNF_SWFLAG) }

fn mdic<B: Bus>(b: &mut B, reg: u32, cmd: u32) -> R<u16, B::Error> {
    b.w32(MDIC, cmd | (PHY_ADDR << MDIC_PHY_SHIFT) | (reg << MDIC_REG_SHIFT))?;
    for _ in 0..1000 {
        b.delay_us(10);
        let v = b.r32(MDIC)?;
        if v & MDIC_ERROR != 0 { return Err(Error::MdicFailed { reg }); }
        if v & MDIC_READY != 0 { return Ok(v as u16); }
    }
    Err(Error::MdicTimeout { reg })
}

/// Read a PHY register (the caller holds the semaphore).
pub fn mdic_read<B: Bus>(b: &mut B, reg: u32) -> R<u16, B::Error> { mdic(b, reg, MDIC_OP_READ) }

/// Write a PHY register (the caller holds the semaphore).
pub fn mdic_write<B: Bus>(b: &mut B, reg: u32, v: u16) -> R<(), B::Error> {
    mdic(b, reg, MDIC_OP_WRITE | u32::from(v)).map(|_| ())
}

/// What `phy_check` found and did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phy {
    /// PHY identifier (MII registers 2 and 3).
    pub id: u32,
    pub bmcr: u16,
    pub bmsr: u16,
    /// Autonegotiation was off, or the PHY powered down or isolated, so
    /// BMCR was rewritten and autonegotiation restarted.
    pub restarted: bool,
    /// The semaphore was taken (false: the firmware never let go).
    pub semaphore: bool,
}

/// Make sure the PHY autonegotiates, is powered and not isolated; a PHY
/// already doing so is left alone (no link drop). Never resets it.
pub fn phy_check<B: Bus>(b: &mut B) -> R<Phy, B::Error> {
    let semaphore = semaphore(b)?;
    let r = phy_fix(b, semaphore);
    release_semaphore(b)?;
    r
}

fn phy_fix<B: Bus>(b: &mut B, semaphore: bool) -> R<Phy, B::Error> {
    let id = (u32::from(mdic_read(b, MII_PHYID1)?) << 16) | u32::from(mdic_read(b, MII_PHYID2)?);
    let bmcr = mdic_read(b, MII_BMCR)?;
    let bmsr = mdic_read(b, MII_BMSR)?;
    let want = (bmcr | BMCR_AUTOEN) & !(BMCR_PDOWN | BMCR_ISOLATE);
    let restarted = want != bmcr;
    if restarted { mdic_write(b, MII_BMCR, want | BMCR_STARTNEG)?; }
    Ok(Phy { id, bmcr, bmsr, restarted, semaphore })
}

/// Stop receive and transmit, mask and acknowledge every interrupt, and
/// give in-flight DMA 10 ms to finish.
pub fn disable<B: Bus>(b: &mut B) -> R<(), B::Error> {
    b.w32(IMC, u32::MAX)?;
    b.w32(RCTL, 0)?;
    b.w32(TCTL, TCTL_PSP)?;
    b.r32(STATUS)?; // flush the posted writes
    b.delay_us(10_000);
    b.w32(IMC, u32::MAX)?;
    b.r32(ICR)?;
    Ok(())
}

/// A MAC reset (CTRL.RST; never PHY_RST): bus mastering stopped first
/// (CTRL.GIO_MASTER_DISABLE until STATUS.GIO_MASTER_EN clears, up to 10 ms),
/// the semaphore held across it, then up to 100 ms for RST to clear, and on
/// a PCH part up to 150 ms for the NVM reload (STATUS.LAN_INIT_DONE). It
/// clears the rings' registers and RCTL/TCTL; the PHY and its link stay.
pub fn reset<B: Bus>(b: &mut B, kind: Kind) -> R<(), B::Error> {
    set32(b, CTRL, CTRL_GIO_MASTER_DISABLE)?;
    wait(b, 100, 100, |b| Ok(b.r32(STATUS)? & STATUS_GIO_MASTER_EN == 0))?;
    semaphore(b)?;
    let ctrl = b.r32(CTRL)?;
    b.w32(CTRL, ctrl | CTRL_RST)?;
    // No register access for a while after RST (the MAC is reloading).
    b.delay_us(20_000);
    let done = wait(b, 100, 1000, |b| Ok(b.r32(CTRL)? & CTRL_RST == 0))?;
    release_semaphore(b)?;
    if !done { return Err(Error::ResetTimeout { ctrl: b.r32(CTRL)? }); }
    if kind == Kind::Pch && wait(b, 150, 1000, |b| Ok(b.r32(STATUS)? & STATUS_LAN_INIT_DONE != 0))? {
        clr32(b, STATUS, STATUS_LAN_INIT_DONE)?;
    }
    b.w32(IMC, u32::MAX)?;
    b.r32(ICR)?;
    Ok(())
}

/// Quiesce: RX/TX off, interrupts masked, the MAC reset, and the management
/// firmware told no driver has the NIC (CTRL_EXT.DRV_LOAD clear). After it
/// the NIC does no DMA. Every path that leaves the NIC calls this.
pub fn quiesce<B: Bus>(b: &mut B, kind: Kind) -> R<(), B::Error> {
    disable(b)?;
    reset(b, kind)?;
    clr32(b, CTRL_EXT, CTRL_EXT_DRV_LOAD)
}

/// Where `mac_init` points the controller.
#[derive(Clone, Copy, Debug)]
pub struct Rings {
    pub rx: u64,
    pub rx_count: u32,
    pub tx: u64,
    pub tx_count: u32,
}

/// Program the MAC after `reset`: link set up from the PHY (SLU, ASDE, no
/// forced speed or duplex), flow control off, interrupts masked, the
/// station address in receive address 0, the multicast table empty, the
/// rings (every receive descriptor but one handed to the hardware), the
/// transmit settings and the receive filter (station, broadcast, every
/// multicast; promiscuous on request, CRC stripped). RX and TX stay off.
pub fn mac_init<B: Bus>(b: &mut B, kind: Kind, mac: [u8; 6], rings: Rings, promiscuous: bool) -> R<(), B::Error> {
    let ctrl = b.r32(CTRL)?;
    let clear = CTRL_LRST | CTRL_ILOS | CTRL_FRCSPD | CTRL_FRCDPLX | CTRL_RFCE | CTRL_TFCE | CTRL_VME
        | CTRL_PHY_RST | CTRL_RST | CTRL_GIO_MASTER_DISABLE;
    b.w32(CTRL, (ctrl & !clear) | CTRL_SLU | CTRL_ASDE)?;
    set32(b, CTRL_EXT, CTRL_EXT_DRV_LOAD)?;
    // Flow control off: no pause frame is ever sent, none honoured.
    for r in [FCAL, FCAH, FCT, FCTTV, FCRTL, FCRTH] { b.w32(r, 0)?; }
    b.w32(IMC, u32::MAX)?;
    b.r32(ICR)?;
    b.w32(RAL0, u32::from_le_bytes([mac[0], mac[1], mac[2], mac[3]]))?;
    b.w32(RAH0, u32::from(mac[4]) | (u32::from(mac[5]) << 8) | RAH_AV)?;
    for i in 0..MTA_COUNT { b.w32(MTA + 4 * i, 0)?; }
    let txdctl = b.r32(TXDCTL)? & !(TXDCTL_PTHRESH | TXDCTL_WTHRESH);
    let count = if kind == Kind::M82574 { TXDCTL_COUNT_DESC } else { 0 };
    b.w32(TXDCTL, txdctl | TXDCTL_GRAN | (1 << 16) | 0x1f | count)?;
    b.w32(RDBAL, rings.rx as u32)?;
    b.w32(RDBAH, (rings.rx >> 32) as u32)?;
    b.w32(RDLEN, rings.rx_count * 16)?;
    b.w32(RDH, 0)?;
    b.w32(RDT, rings.rx_count - 1)?;
    b.w32(TDBAL, rings.tx as u32)?;
    b.w32(TDBAH, (rings.tx >> 32) as u32)?;
    b.w32(TDLEN, rings.tx_count * 16)?;
    b.w32(TDH, 0)?;
    b.w32(TDT, 0)?;
    b.w32(TIPG, TIPG_COPPER)?;
    b.w32(TCTL, TCTL_PSP | TCTL_CT | TCTL_COLD | TCTL_RTLC)?;
    b.w32(RCTL, rctl(promiscuous))?;
    Ok(())
}

/// RCTL without EN: broadcast, every multicast, CRC stripped, 2 KiB
/// buffers; every unicast too with `promiscuous`.
pub fn rctl(promiscuous: bool) -> u32 {
    RCTL_BAM | RCTL_MPE | RCTL_SECRC | if promiscuous { RCTL_UPE } else { 0 }
}

/// Change only the unicast-promiscuous bit; RX stays as it was.
pub fn set_filter<B: Bus>(b: &mut B, promiscuous: bool) -> R<(), B::Error> {
    let en = b.r32(RCTL)? & RCTL_EN;
    Ok(b.w32(RCTL, rctl(promiscuous) | en)?)
}

/// Turn RX and TX on, after the rings are filled.
pub fn enable<B: Bus>(b: &mut B) -> R<(), B::Error> {
    set32(b, TCTL, TCTL_EN)?;
    set32(b, RCTL, RCTL_EN)
}

/// Wait up to `ms` for the link; the last state read.
pub fn wait_link<B: Bus>(b: &mut B, ms: usize) -> R<Link, B::Error> {
    let mut l = link(b)?;
    for _ in 0..ms / 10 {
        if l.up { break; }
        b.delay_us(10_000);
        l = link(b)?;
    }
    Ok(l)
}
