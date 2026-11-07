//! stormnic-e1000e: an `EFI_SIMPLE_NETWORK_PROTOCOL` driver for the Intel
//! e1000e family (82574L, 82579, I217, I218, I219), in Rust.
//!
//! An EFI boot-service driver (see build.rs). The entry point installs an
//! `EFI_DRIVER_BINDING_PROTOCOL` on the image handle (binding.rs) and returns;
//! the firmware's `ConnectController` then calls Supported/Start for each
//! controller. Start reads the station address, makes sure the PHY
//! autonegotiates, brings the NIC up once and runs a DMA check when the link
//! is up, quiesces it again and puts an `EFI_SIMPLE_NETWORK_PROTOCOL` on a
//! child handle; the user (stormbootx's smoltcp) opens it EXCLUSIVE, and
//! only its Initialize turns RX/TX on.
#![no_main]
#![no_std]

extern crate alloc;

// First: its macros (say!, trace!, fail!, note!) are used by the modules below.
#[macro_use]
mod console;
mod binding;
mod decode;
mod e1000e;
mod ids;
mod pci_io;
mod snp;
mod trace;

use uefi::prelude::*;

#[entry]
fn main() -> Status {
    if uefi::helpers::init().is_err() {
        return Status::LOAD_ERROR;
    }
    console::init();
    log::set_max_level(if console::verbose() { log::LevelFilter::Info } else { log::LevelFilter::Warn });
    let version = env!("CARGO_PKG_VERSION");
    match binding::install() {
        Ok(()) => {
            trace!(
                "stormnic-e1000e {version}: driver binding installed ({} Intel e1000e device IDs), flow control off",
                ids::SUPPORTED.len()
            );
            Status::SUCCESS
        }
        Err(e) => {
            say!("stormnic-e1000e {version}: driver binding not installed: {:?}", e.status());
            e.status()
        }
    }
}
