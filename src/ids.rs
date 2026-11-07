//! The PCI functions this driver binds, checked against pci.ids
//! (pci-ids.ucw.cz, version 2026.11.06); the names are pci.ids'.
//!
//! Two kinds, which differ in where the MAC address and the NVM are and in
//! how the PHY is shared (docs/hardware.md):
//!
//! - `M82574`: the discrete 82574L, a PCIe card or on-board chip with its
//!   own EEPROM behind EERD. QEMU's `e1000e` device is one (8086:10d3).
//! - `Pch`: the LAN controller inside an Intel chipset (82579 with its PHY on
//!   6/7-series, I217/I218/I219 on 8-series and later, including the
//!   400/500-series IDs pci.ids names by chipset). Its NVM is a region of the
//!   platform's SPI flash and the PHY is shared with the management engine.
//!
//! Not here: the 82571/82572/82573/82583 and the ICH8–ICH10 82566/82567/
//! 82577/82578 (older, different NVM and PHY handling), and the I225/I226
//! (igc, a different register set).

use crate::e1000e::Kind;

/// PCI class: network controller.
pub const CLASS_NETWORK: u8 = 0x02;

pub struct Nic {
    pub device: u16,
    pub kind: Kind,
    pub name: &'static str,
}

pub const SUPPORTED: &[Nic] = &[
    Nic { device: 0x0d4c, kind: Kind::Pch, name: "400 Series Chipset Family GbE Controller (Corporate/vPro)" },
    Nic { device: 0x0d4d, kind: Kind::Pch, name: "400 Series Chipset Family GbE Controller (Consumer)" },
    Nic { device: 0x0d4e, kind: Kind::Pch, name: "400 Series Chipset Family On-Package GbE Controller (Corporate/vPro)" },
    Nic { device: 0x0d4f, kind: Kind::Pch, name: "400 Series Chipset Family On-Package GbE Controller (Consumer)" },
    Nic { device: 0x0d53, kind: Kind::Pch, name: "Ethernet Connection (12) I219-LM" },
    Nic { device: 0x0d55, kind: Kind::Pch, name: "Ethernet Connection (12) I219-V" },
    Nic { device: 0x0dc5, kind: Kind::Pch, name: "Ethernet Connection (23) I219-LM" },
    Nic { device: 0x0dc6, kind: Kind::Pch, name: "Ethernet Connection (23) I219-V" },
    Nic { device: 0x0dc7, kind: Kind::Pch, name: "Ethernet Connection I219-LM" },
    Nic { device: 0x0dc8, kind: Kind::Pch, name: "Ethernet Connection I219-V" },
    Nic { device: 0x10d3, kind: Kind::M82574, name: "82574L Gigabit Network Connection" },
    Nic { device: 0x10f6, kind: Kind::M82574, name: "82574L Gigabit Network Connection" },
    Nic { device: 0x1502, kind: Kind::Pch, name: "82579LM Gigabit Network Connection (Lewisville)" },
    Nic { device: 0x1503, kind: Kind::Pch, name: "82579V Gigabit Network Connection" },
    Nic { device: 0x153a, kind: Kind::Pch, name: "Ethernet Connection I217-LM" },
    Nic { device: 0x153b, kind: Kind::Pch, name: "Ethernet Connection I217-V" },
    Nic { device: 0x1559, kind: Kind::Pch, name: "Ethernet Connection I218-V" },
    Nic { device: 0x155a, kind: Kind::Pch, name: "Ethernet Connection I218-LM" },
    Nic { device: 0x156f, kind: Kind::Pch, name: "Ethernet Connection I219-LM" },
    Nic { device: 0x1570, kind: Kind::Pch, name: "Ethernet Connection I219-V" },
    Nic { device: 0x15a0, kind: Kind::Pch, name: "Ethernet Connection (2) I218-LM" },
    Nic { device: 0x15a1, kind: Kind::Pch, name: "Ethernet Connection (2) I218-V" },
    Nic { device: 0x15a2, kind: Kind::Pch, name: "Ethernet Connection (3) I218-LM" },
    Nic { device: 0x15a3, kind: Kind::Pch, name: "Ethernet Connection (3) I218-V" },
    Nic { device: 0x15b7, kind: Kind::Pch, name: "Ethernet Connection (2) I219-LM" },
    Nic { device: 0x15b8, kind: Kind::Pch, name: "Ethernet Connection (2) I219-V" },
    Nic { device: 0x15b9, kind: Kind::Pch, name: "Ethernet Connection (3) I219-LM" },
    Nic { device: 0x15bb, kind: Kind::Pch, name: "Ethernet Connection (7) I219-LM" },
    Nic { device: 0x15bc, kind: Kind::Pch, name: "Ethernet Connection (7) I219-V" },
    Nic { device: 0x15bd, kind: Kind::Pch, name: "Ethernet Connection (6) I219-LM" },
    Nic { device: 0x15be, kind: Kind::Pch, name: "Ethernet Connection (6) I219-V" },
    Nic { device: 0x15d6, kind: Kind::Pch, name: "Ethernet Connection (5) I219-V" },
    Nic { device: 0x15d7, kind: Kind::Pch, name: "Ethernet Connection (4) I219-LM" },
    Nic { device: 0x15d8, kind: Kind::Pch, name: "Ethernet Connection (4) I219-V" },
    Nic { device: 0x15df, kind: Kind::Pch, name: "Ethernet Connection (8) I219-LM" },
    Nic { device: 0x15e0, kind: Kind::Pch, name: "Ethernet Connection (8) I219-V" },
    Nic { device: 0x15e1, kind: Kind::Pch, name: "Ethernet Connection (9) I219-LM" },
    Nic { device: 0x15e2, kind: Kind::Pch, name: "Ethernet Connection (9) I219-V" },
    Nic { device: 0x15e3, kind: Kind::Pch, name: "Ethernet Connection (5) I219-LM" },
    Nic { device: 0x15f4, kind: Kind::Pch, name: "Ethernet Connection (15) I219-LM" },
    Nic { device: 0x15f5, kind: Kind::Pch, name: "Ethernet Connection (15) I219-V" },
    Nic { device: 0x15f9, kind: Kind::Pch, name: "500 Series Chipset Family GbE Controller (Corporate/vPro)" },
    Nic { device: 0x15fa, kind: Kind::Pch, name: "500 Series Chipset Family GbE Controller (Consumer)" },
    Nic { device: 0x15fb, kind: Kind::Pch, name: "Ethernet Connection (13) I219-LM" },
    Nic { device: 0x15fc, kind: Kind::Pch, name: "Ethernet Connection (13) I219-V" },
    Nic { device: 0x1a1c, kind: Kind::Pch, name: "Ethernet Connection (17) I219-LM" },
    Nic { device: 0x1a1d, kind: Kind::Pch, name: "Ethernet Connection (17) I219-V" },
    Nic { device: 0x1a1e, kind: Kind::Pch, name: "Ethernet Connection (16) I219-LM" },
    Nic { device: 0x1a1f, kind: Kind::Pch, name: "Ethernet Connection (16) I219-V" },
    Nic { device: 0x550a, kind: Kind::Pch, name: "Ethernet Connection (18) I219-LM" },
    Nic { device: 0x550b, kind: Kind::Pch, name: "Ethernet Connection (18) I219-LM" },
    Nic { device: 0x550e, kind: Kind::Pch, name: "Ethernet Connection (20) I219-LM" },
    Nic { device: 0x550f, kind: Kind::Pch, name: "Ethernet Connection (20) I219-V" },
    Nic { device: 0x5510, kind: Kind::Pch, name: "Ethernet Connection (21) I219-LM" },
    Nic { device: 0x5511, kind: Kind::Pch, name: "Ethernet Connection (21) I219-V" },
    Nic { device: 0x57a0, kind: Kind::Pch, name: "Ethernet Connection (24) I219-LM" },
    Nic { device: 0x57a1, kind: Kind::Pch, name: "Ethernet Connection (24) I219-V" },
    Nic { device: 0x57b3, kind: Kind::Pch, name: "Ethernet Connection (25) I219-LM" },
    Nic { device: 0x57b4, kind: Kind::Pch, name: "Ethernet Connection (25) I219-V" },
    Nic { device: 0x57b5, kind: Kind::Pch, name: "Ethernet Connection (26) I219-LM" },
    Nic { device: 0x57b6, kind: Kind::Pch, name: "Ethernet Connection (26) I219-V" },
    Nic { device: 0x57b7, kind: Kind::Pch, name: "Ethernet Connection (27) I219-LM" },
    Nic { device: 0x57b8, kind: Kind::Pch, name: "Ethernet Connection (27) I219-V" },
    Nic { device: 0x57b9, kind: Kind::Pch, name: "Ethernet Connection (29) I219-LM" },
    Nic { device: 0x57ba, kind: Kind::Pch, name: "Ethernet Connection (29) I219-V" },
];

pub fn lookup(vendor: u16, device: u16) -> Option<&'static Nic> {
    if vendor != crate::e1000e::VENDOR {
        return None;
    }
    SUPPORTED.iter().find(|n| n.device == device)
}
