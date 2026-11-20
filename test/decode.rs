//! Memory decode and bus mastering (`src/decode.rs`) against a fake PciIo
//! whose attribute calls drive a command register the way EDK2's do.
#[path = "../src/decode.rs"]
#[allow(dead_code)]
mod decode;
use decode::*;
use std::cell::Cell;

/// A function's command register and its PciIo attribute operations:
/// `Enable`/`Disable`/`Set` move MSE and BME as firmware does. `refuse`
/// makes every attribute call fail (AMI Aptio 4, decode.rs).
struct Fake {
    command: Cell<u16>,
    refuse: bool,
}

const MEM: u16 = COMMAND_MEMORY;
const BM: u16 = COMMAND_BUS_MASTER;

fn bits(attributes: u64) -> u16 {
    (if attributes & ATTRIBUTE_MEMORY != 0 { MEM } else { 0 })
        | (if attributes & ATTRIBUTE_BUS_MASTER != 0 { BM } else { 0 })
}

impl Fake {
    fn new(command: u16, refuse: bool) -> Fake { Fake { command: Cell::new(command), refuse } }
    fn attrs(&self) -> u64 {
        let c = self.command.get();
        (if c & MEM != 0 { ATTRIBUTE_MEMORY } else { 0 }) | (if c & BM != 0 { ATTRIBUTE_BUS_MASTER } else { 0 })
    }
    fn ok(&self) -> Result<(), ()> { if self.refuse { Err(()) } else { Ok(()) } }
}

impl Pci for Fake {
    type Status = ();
    fn get(&self) -> Result<u64, ()> { self.ok().map(|_| self.attrs()) }
    fn supported(&self) -> Result<u64, ()> { self.ok().map(|_| ATTRIBUTE_MEMORY | ATTRIBUTE_BUS_MASTER) }
    fn enable(&self, a: u64) -> Result<(), ()> {
        self.ok()?;
        self.command.set(self.command.get() | bits(a));
        Ok(())
    }
    fn disable(&self, a: u64) -> Result<(), ()> {
        self.ok()?;
        self.command.set(self.command.get() & !bits(a));
        Ok(())
    }
    fn set(&self, a: u64) -> Result<(), ()> {
        self.ok()?;
        self.command.set((self.command.get() & !(MEM | BM)) | bits(a));
        Ok(())
    }
    fn config_read_u16(&self, offset: u32) -> Result<u16, ()> {
        assert_eq!(offset, COMMAND);
        Ok(self.command.get())
    }
    fn config_write_u16(&self, offset: u32, value: u16) -> Result<(), ()> {
        assert_eq!(offset, COMMAND);
        self.command.set(value);
        Ok(())
    }
}

#[test]
fn enable_sets_both_and_release_puts_them_back() {
    for (before, refuse) in [(0, false), (MEM | BM, false), (0, true), (MEM | BM, true), (MEM, true)] {
        let f = Fake::new(before, refuse);
        let e = enable(&f).unwrap();
        assert_eq!(f.command.get() & (MEM | BM), MEM | BM);
        release(&f, &e).unwrap();
        assert_eq!(f.command.get(), before, "before {before:#x}, attributes refused {refuse}");
    }
}

/// Finding cc9c5c400a6b: after a quiesce that failed, the put-back must not
/// turn bus mastering on again, even when it was on before Start (by the
/// attributes or by the command register).
#[test]
fn release_stopped_never_puts_bus_mastering_back() {
    for (before, refuse) in [(0, false), (MEM | BM, false), (MEM, false), (0, true), (MEM | BM, true), (MEM, true)] {
        let f = Fake::new(before, refuse);
        let e = enable(&f).unwrap();
        // keep_dma's order: nothing else ran between.
        let r = release_stopped(&f, &e);
        assert_eq!(r.master_off, Ok(()));
        assert_eq!(f.command.get() & BM, 0, "BME clear: before {before:#x}, attributes refused {refuse}");
        assert_eq!(f.command.get() & MEM, before & MEM, "memory decode as Start found it");
    }
}

#[test]
fn release_after_release_stopped_is_what_the_old_cleanup_did() {
    // The defect the finding names: stop_bus_master, then the plain
    // release, which turns BME back on when it was on before Start.
    let f = Fake::new(MEM | BM, false);
    let e = enable(&f).unwrap();
    stop_bus_master(&f).unwrap();
    release(&f, &e).unwrap();
    assert_eq!(f.command.get() & BM, BM, "the plain release restores BME");
    // release_stopped does not.
    let f = Fake::new(MEM | BM, false);
    let e = enable(&f).unwrap();
    let _ = release_stopped(&f, &e);
    assert_eq!(f.command.get() & BM, 0);
}

#[test]
fn stop_bus_master_clears_bme_when_the_attribute_call_is_refused() {
    let f = Fake::new(MEM | BM, true);
    stop_bus_master(&f).unwrap();
    assert_eq!(f.command.get(), MEM);
}
