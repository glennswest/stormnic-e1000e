//! The MAC/PHY sequences and the rings against the simulated controller in
//! `sim.rs`.
#[path = "../src/e1000e.rs"]
#[allow(dead_code)]
mod e1000e;
#[allow(dead_code)]
mod sim;
use e1000e::ring::{self, RX_COUNT, TX_COUNT};
use e1000e::{self as nic, Bus, Error, Kind, Link, MacSource};
use sim::*;

const KINDS: [Kind; 2] = [Kind::M82574, Kind::Pch];

#[test]
fn identify_reads_the_address_and_link_and_writes_nothing() {
    for kind in KINDS {
        let mut d = Dev::new(kind);
        let id = nic::identify(&mut d, kind).unwrap();
        assert_eq!((id.mac, id.mac_from), (MAC, MacSource::Ral0));
        assert!(id.link.up);
        assert_eq!(id.link.mbps, 1000);
        assert!(d.ops.is_empty(), "{kind:?}: identify writes nothing");
    }
}

#[test]
fn an_empty_receive_address_falls_back_to_the_82574_eeprom_only() {
    let mut d = Dev::new(Kind::M82574);
    d.put(nic::RAH0, 0);
    assert_eq!(nic::read_mac(&mut d, Kind::M82574), Ok((MAC, MacSource::Eeprom)));
    let mut d = Dev::new(Kind::Pch);
    d.put(nic::RAH0, 0);
    assert_eq!(nic::read_mac(&mut d, Kind::Pch), Err(Error::NoMac));
    let mut d = Dev::new(Kind::M82574);
    d.put(nic::RAH0, 0);
    d.eeprom[..3].fill(0xffff);
    assert_eq!(nic::read_mac(&mut d, Kind::M82574), Err(Error::NoMac));
}

#[test]
fn a_dead_function_is_removed() {
    // A BAR that reads all ones.
    struct Gone;
    impl Bus for Gone {
        type Error = ();
        fn r32(&mut self, _: u32) -> Result<u32, ()> { Ok(u32::MAX) }
        fn w32(&mut self, _: u32, _: u32) -> Result<(), ()> { Ok(()) }
        fn delay_us(&mut self, _: usize) {}
    }
    assert_eq!(nic::identify(&mut Gone, Kind::M82574), Err(Error::Removed));
}

#[test]
fn status_decodes_speed_duplex_and_the_mac_s_pause() {
    let up = nic::STATUS_LU | nic::STATUS_FD | (2 << nic::STATUS_SPEED_SHIFT);
    let l = Link::from_regs(up, 0);
    assert_eq!(l, Link { up: true, mbps: 1000, full_duplex: true, rx_pause: false, tx_pause: false });
    assert_eq!(format!("{l}"), "up 1000 Mb/s full duplex, flow control off");
    let l = Link::from_regs(nic::STATUS_LU | (1 << nic::STATUS_SPEED_SHIFT), nic::CTRL_TFCE);
    assert_eq!(format!("{l}"), "up 100 Mb/s half duplex, flow control rx off tx on");
    assert_eq!(format!("{}", Link::from_regs(0, 0)), "down");
}

#[test]
fn start_programs_rings_filter_and_turns_flow_control_off() {
    for kind in KINDS {
        let mut d = Dev::new(kind);
        assert!(nic::link(&mut d).unwrap().tx_pause, "the firmware left pause on");
        let mut net = d.net();
        net.start(&mut d, false).unwrap();
        assert!(d.rx_enabled() && d.tx_enabled());
        let rings = net.rings();
        assert_eq!(u64::from(d.get(nic::RDBAL)) | u64::from(d.get(nic::RDBAH)) << 32, rings.rx);
        assert_eq!(u64::from(d.get(nic::TDBAL)) | u64::from(d.get(nic::TDBAH)) << 32, rings.tx);
        assert_eq!(d.get(nic::RDLEN), RX_COUNT as u32 * 16);
        assert_eq!(d.get(nic::TDLEN), TX_COUNT as u32 * 16);
        assert_eq!((d.get(nic::RDH), d.get(nic::RDT)), (0, RX_COUNT as u32 - 1));
        assert_eq!((d.get(nic::TDH), d.get(nic::TDT)), (0, 0));
        let rctl = d.get(nic::RCTL);
        assert_eq!(rctl & (nic::RCTL_BAM | nic::RCTL_MPE | nic::RCTL_SECRC), nic::RCTL_BAM | nic::RCTL_MPE | nic::RCTL_SECRC);
        assert_eq!(rctl & nic::RCTL_UPE, 0);
        let ctrl = d.get(nic::CTRL);
        assert_eq!(ctrl & (nic::CTRL_SLU | nic::CTRL_ASDE), nic::CTRL_SLU | nic::CTRL_ASDE);
        assert_eq!(ctrl & (nic::CTRL_TFCE | nic::CTRL_RFCE | nic::CTRL_FRCSPD | nic::CTRL_FRCDPLX), 0);
        for r in [nic::FCAL, nic::FCAH, nic::FCT, nic::FCTTV, nic::FCRTL, nic::FCRTH] { assert_eq!(d.get(r), 0, "{r:#x}"); }
        assert_ne!(d.get(nic::CTRL_EXT) & nic::CTRL_EXT_DRV_LOAD, 0);
        assert_eq!(d.get(nic::IMS), 0, "no interrupt enabled");
        assert_eq!(d.get(nic::RAH0) & nic::RAH_AV, nic::RAH_AV);
        let l = nic::link(&mut d).unwrap();
        assert!(l.up && !l.tx_pause && !l.rx_pause, "{kind:?}: {l}");
        let txdctl = d.get(nic::TXDCTL);
        assert_eq!(txdctl & nic::TXDCTL_COUNT_DESC != 0, kind == Kind::M82574);
    }
}

#[test]
fn the_phy_is_never_reset() {
    let (mut d, mut net) = started(Kind::M82574);
    net.stop(&mut d).unwrap();
    assert!(d.writes(nic::CTRL).iter().all(|v| v & nic::CTRL_PHY_RST == 0));
    assert!(d.resets >= 2, "MAC resets at start and stop");
    assert_eq!(d.an_restarts, 0);
}

#[test]
fn promiscuous_sets_upe_and_keeps_rx_on() {
    let (mut d, net) = started(Kind::M82574);
    net.set_promiscuous(&mut d, true).unwrap();
    assert_eq!(d.get(nic::RCTL) & (nic::RCTL_UPE | nic::RCTL_EN), nic::RCTL_UPE | nic::RCTL_EN);
    net.set_promiscuous(&mut d, false).unwrap();
    assert_eq!(d.get(nic::RCTL) & (nic::RCTL_UPE | nic::RCTL_EN), nic::RCTL_EN);
}

#[test]
fn transmit_pads_and_reclaims() {
    let (mut d, mut net) = started(Kind::M82574);
    let f = frame_to([0xff; 6], 1);
    assert!(net.transmit(&mut d, &f[..20]).unwrap());
    assert_eq!(d.wire[0].len(), 60);
    assert_eq!(&d.wire[0][..20], &f[..20]);
    assert!(d.wire[0][20..].iter().all(|b| *b == 0));
    net.reclaim();
    assert_eq!(net.tx_pending(), 0);
    assert_eq!(net.transmit(&mut d, &[0u8; 13]), Err(Error::Frame { len: 13 }));
    assert_eq!(net.transmit(&mut d, &[0u8; 1519]), Err(Error::Frame { len: 1519 }));
}

#[test]
fn a_full_transmit_ring_says_so_and_never_lets_tdt_reach_tdh() {
    let (mut d, mut net) = started(Kind::M82574);
    d.tx_stalled = true;
    let f = frame_to([0xff; 6], 1);
    for _ in 0..TX_COUNT - 1 { assert!(net.transmit(&mut d, &f).unwrap()); }
    assert!(!net.transmit(&mut d, &f).unwrap());
    assert_ne!(d.get(nic::TDT), d.get(nic::TDH));
    d.tx_stalled = false;
    d.w32(nic::TDT, d.get(nic::TDT)).unwrap();
    assert_eq!(d.wire.len(), TX_COUNT - 1);
    assert!(net.transmit(&mut d, &f).unwrap());
}

#[test]
fn frames_come_back_in_order_across_many_wraps() {
    let (mut d, mut net) = started(Kind::Pch);
    d.loopback = true;
    let mut buf = [0u8; ring::BUF_SIZE];
    for i in 0..200u32 {
        let f = frame_to(MAC, i as u8);
        assert!(net.transmit(&mut d, &f).unwrap());
        assert_eq!(net.receive(&mut d, &mut buf), Ok(Some(60)), "frame {i}");
        assert_eq!(buf[14], i as u8);
    }
    assert_eq!(net.receive(&mut d, &mut buf), Ok(None));
    assert_eq!((d.dropped, net.rx_errors), (0, 0));
}

#[test]
fn the_receive_ring_holds_count_minus_one_then_drops() {
    let (mut d, mut net) = started(Kind::M82574);
    for i in 0..RX_COUNT + 3 { d.deliver(&frame_to(MAC, i as u8)); }
    assert_eq!(d.dropped, 4);
    let mut buf = [0u8; ring::BUF_SIZE];
    for i in 0..RX_COUNT - 1 {
        assert_eq!(net.receive(&mut d, &mut buf), Ok(Some(60)));
        assert_eq!(buf[14], i as u8);
    }
    assert_eq!(net.receive(&mut d, &mut buf), Ok(None));
    // Every descriptor went back: the ring takes a full load again.
    for _ in 0..RX_COUNT - 1 { d.deliver(&frame_to(MAC, 0)); }
    assert_eq!(d.dropped, 4);
}

#[test]
fn errored_split_and_runt_descriptors_are_skipped_and_given_back() {
    let (mut d, mut net) = started(Kind::M82574);
    d.deliver_raw(60, ring::DD | ring::RX_EOP | (0x01 << 8)); // CRC error
    d.deliver_raw(60, ring::DD); // no EOP: split
    d.deliver_raw(10, ring::DD | ring::RX_EOP); // runt
    d.deliver(&frame_to([0xff; 6], 7));
    let mut buf = [0u8; ring::BUF_SIZE];
    assert_eq!(net.receive(&mut d, &mut buf), Ok(Some(60)));
    assert_eq!(buf[14], 7);
    assert_eq!(net.rx_errors, 3);
    assert_eq!(d.get(nic::RDT), 3, "each one handed back");
}

#[test]
fn a_frame_too_big_for_the_buffer_stays_queued() {
    let (mut d, mut net) = started(Kind::M82574);
    d.deliver(&frame_to(MAC, 1));
    assert_eq!(net.receive(&mut d, &mut [0u8; 30]), Err(Error::Frame { len: 60 }));
    assert_eq!(net.receive(&mut d, &mut [0u8; 60]), Ok(Some(60)));
}

#[test]
fn stop_quiesces_and_tells_the_firmware() {
    for kind in KINDS {
        let (mut d, mut net) = started(kind);
        let resets = d.resets;
        net.stop(&mut d).unwrap();
        assert!(!net.running);
        assert!(!d.rx_enabled() && !d.tx_enabled());
        assert_eq!(d.resets, resets + 1);
        assert_eq!(d.get(nic::CTRL_EXT) & nic::CTRL_EXT_DRV_LOAD, 0);
        assert_eq!(*d.writes(nic::IMC).last().unwrap(), u32::MAX);
        d.deliver(&frame_to(MAC, 1));
        assert_eq!(d.dropped, 1, "nothing is received once stopped");
        let mut buf = [0u8; 64];
        assert_eq!(net.receive(&mut d, &mut buf), Ok(None));
    }
}

#[test]
fn a_reset_that_never_completes_fails_start() {
    let mut d = Dev::new(Kind::M82574);
    d.reset_stuck = true;
    let mut net = d.net();
    assert!(matches!(net.start(&mut d, false), Err(Error::ResetTimeout { .. })));
    assert!(!net.running);
    assert!(!d.rx_enabled() && !d.tx_enabled());
}

#[test]
fn the_pch_reset_waits_for_the_nvm_reload_and_acknowledges_it() {
    let (d, _net) = started(Kind::Pch);
    assert_eq!(d.get(nic::STATUS) & nic::STATUS_LAN_INIT_DONE, 0);
    assert!(d.writes(nic::STATUS).iter().all(|v| v & nic::STATUS_LAN_INIT_DONE == 0));
    let mut d = Dev::new(Kind::M82574);
    let mut net = d.net();
    net.start(&mut d, false).unwrap();
    assert!(d.writes(nic::STATUS).is_empty(), "the 82574 has no LAN_INIT_DONE");
}

#[test]
fn reset_takes_and_releases_the_phy_semaphore() {
    let (d, _net) = started(Kind::Pch);
    let w = d.writes(nic::EXTCNF_CTRL);
    assert!(w.iter().any(|v| v & nic::EXTCNF_SWFLAG != 0));
    assert_eq!(d.get(nic::EXTCNF_CTRL) & nic::EXTCNF_SWFLAG, 0);
}

#[test]
fn phy_check_leaves_an_autonegotiating_phy_alone() {
    let mut d = Dev::new(Kind::M82574);
    let p = nic::phy_check(&mut d).unwrap();
    assert_eq!((p.id, p.bmcr, p.restarted, p.semaphore), (PHY_ID, 0x1140, false, true));
    assert_eq!(d.an_restarts, 0);
    assert!(nic::link(&mut d).unwrap().up);
    assert_eq!(d.get(nic::EXTCNF_CTRL) & nic::EXTCNF_SWFLAG, 0, "released");
}

#[test]
fn phy_check_powers_up_and_restarts_autonegotiation() {
    let mut d = Dev::new(Kind::Pch);
    d.phy.insert(nic::MII_BMCR, 0x0940); // autoneg off, powered down
    assert!(!nic::link(&mut d).unwrap().up);
    let p = nic::phy_check(&mut d).unwrap();
    assert!(p.restarted);
    assert_eq!(d.an_restarts, 1);
    let bmcr = d.phy[&nic::MII_BMCR];
    assert_eq!(bmcr & (nic::BMCR_AUTOEN | nic::BMCR_PDOWN | nic::BMCR_ISOLATE), nic::BMCR_AUTOEN);
    assert!(!nic::wait_link(&mut d, 1000).unwrap().up);
    assert!(nic::wait_link(&mut d, 3000).unwrap().up);
}

#[test]
fn phy_failures_are_reported_and_the_semaphore_released() {
    let mut d = Dev::new(Kind::M82574);
    d.phy_absent = true;
    assert_eq!(nic::phy_check(&mut d), Err(Error::MdicFailed { reg: nic::MII_PHYID1 }));
    assert_eq!(d.get(nic::EXTCNF_CTRL) & nic::EXTCNF_SWFLAG, 0);
    let mut d = Dev::new(Kind::M82574);
    d.mdic_stuck = true;
    assert_eq!(nic::phy_check(&mut d), Err(Error::MdicTimeout { reg: nic::MII_PHYID1 }));
    let mut d = Dev::new(Kind::Pch);
    d.fw_semaphore = true;
    let p = nic::phy_check(&mut d).unwrap();
    assert!(!p.semaphore, "the firmware never let go; the access went on");
}

#[test]
fn the_dma_check_sees_the_descriptor_written_back() {
    let (mut d, mut net) = started(Kind::M82574);
    assert_eq!(ring::check(&mut d, &mut net), Ok(Some(0)));
    assert_eq!(&d.wire[0][..6], &[0xff; 6]);
    assert_eq!(u16::from_be_bytes([d.wire[0][12], d.wire[0][13]]), 0x88b5);
    let (mut d, mut net) = started(Kind::M82574);
    d.tx_stalled = true;
    assert_eq!(ring::check(&mut d, &mut net), Ok(None));
}

#[test]
fn every_supported_id_is_intel_and_listed_once() {
    // ids.rs is UEFI-independent apart from its path to Kind; read it as text.
    let text = include_str!("../src/ids.rs");
    let ids: Vec<&str> = text.lines().filter_map(|l| l.trim().strip_prefix("Nic { device: 0x")).map(|l| &l[..4]).collect();
    assert!(ids.len() >= 60);
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "a device ID listed twice");
    assert!(ids.contains(&"10d3"), "QEMU's e1000e");
}
