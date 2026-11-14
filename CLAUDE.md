# CLAUDE.md — stormnic-e1000e

A `no_std` UEFI driver giving firmware an `EFI_SIMPLE_NETWORK_PROTOCOL` for
the **Intel e1000e** gigabit family: the 82574L and the PCH-integrated
82579, I217, I218 and I219 (`src/ids.rs`, checked against pci.ids).
Loaded by stormbootx from `\stormboot\drivers` like its siblings
stormnic-ixgbe, stormnic-mlx4, stormnic-virtio and stormnic-realtek.
Read README.md first.

Read the cross-project rules in `../CLAUDE.md` first. In particular, **build
with `sc-build` after pushing, never on this VM and never as root**, and
scratch files go in `tmp/`.

## Rules for this crate

- **Sources:** register offsets, bit fields and descriptor formats from
  Intel's public datasheets (82574 Family GbE Controller Datasheet; the
  register set is shared by the PCH parts), and the UEFI specification for
  the binding and SNP. iPXE's `intel.c` and Linux's e1000e (both GPL) are
  read for behaviour only, **never translated**: the crate is MIT.
  docs/hardware.md describes the hardware in its own words.
- It is a *driver*: the binary is an EFI boot-service driver (subsystem 11)
  installing `EFI_DRIVER_BINDING_PROTOCOL`. Supported declines a function
  another driver already holds BY_DRIVER: the platform's own driver (Intel's
  UNDI, or QEMU's iPXE option ROM) wins unless a loader disconnects it first
  (stormbootx `prefer_media_drivers = e1000e`, the check app).
- **Flow control is off** (as stormnic-realtek#2): CTRL.TFCE/RFCE clear and
  the XON/XOFF thresholds zero, so a receive ring nobody drains drops frames
  instead of sending pause frames to the switch.
- **The NIC is quiet whenever nobody drives it**: RX/TX disabled, interrupts
  masked, MAC reset at every Start failure, SNP Shutdown/Stop, binding Stop
  and ExitBootServices. Only SNP Initialize enables RX and TX.
- The PHY is never reset (the link the firmware or power-on brought up
  stays); Start only makes sure autonegotiation is on and the PHY powered.
- Same layout as stormnic-realtek: the hardware core (`src/e1000e.rs`,
  `src/ring.rs`, `src/snp_core.rs`) is UEFI-independent and tested on the
  host against a simulated controller (`test/`); `src/binding.rs`,
  `src/snp.rs`, `src/pci_io.rs` are the UEFI glue.
- One line per NIC on the console by default, the trace behind
  `StormnicVerbose` / `--features verbose` (stormnic-ixgbe#22).

## Build

```bash
sc-build scripts/check-driver.sh     # build + clippy -D warnings + PE32+ subsystem 11
sc-build scripts/test-host.sh        # simulated-controller tests
sc-build scripts/test-ovmf.sh        # QEMU e1000e (82574L) + OVMF: the check app leases, pings, echoes
```

## Test

- Host: `test/*.rs`, compiled with plain `rustc --test` (scripts/test-host.sh).
- QEMU/OVMF (`scripts/test-ovmf.sh`): QEMU's `e1000e` device is an 82574L
  (8086:10d3). The check app (`check/`) disconnects whatever firmware driver
  holds it (QEMU's iPXE option ROM in the `ipxe` boot), loads
  `\stormboot\drivers\stormnic-e1000e.efi`, connects it and runs smoltcp over
  its SNP: DHCP, ping the gateway, UDP echo, then prints
  `STORMNIC-E1000E CHECK PASS`.
- The PCH parts (82579, I217–I219) have no emulator; they are verified in
  simulation only until a machine with one boots it.

## Version

`Cargo.toml` → `package.version` (and `check/Cargo.toml`). Current: `v0.2.0`.

## Work plan

- [x] #1 UEFI SNP driver: bind, link, transmit/receive (closed 2026-11-07)
  - [x] Scaffold: workspace (driver + `check/`), build.rs subsystem 11,
        glue from stormnic-realtek, PCI IDs from pci.ids (2026.11.06; 67 IDs)
  - [x] Hardware core (reset, MAC address, PHY check, rings), SNP core (5e6965c)
  - [x] Host tests: 38 pass (`sc-build scripts/test-host.sh` at 321c2a6)
  - [x] `sc-build scripts/check-driver.sh` at e5384c7: 57,856 B, PE32+
        subsystem 11, clippy -D warnings clean on driver and check app, no
        warnings (#2, #3, #4 were the first builds' findings, closed)
  - [x] QEMU/OVMF e1000e (`sc-build scripts/test-ovmf.sh` at e5384c7):
        `plain` (no option ROM) and `ipxe` (efi-e1000e.rom held the NIC,
        disconnected) both PASS: lease 10.0.2.15, ping 5/5, UDP echo 1200 B,
        `link up 1000 Mb/s full duplex, flow control off, SNP installed`
  - [x] check-driver and host tests again at d0bc5ec (adds 550c/550d):
        57,856 B, subsystem 11, sha256 99833a56…32dc; 38 tests pass
  - [x] Pin asked on stormbootx#70 (d0bc5ecb6280c33cc0600f51b98c7c6c5ae954d8
        and an `e1000e` family for `prefer_media_drivers`). stormbootx#70 waits
        on #1 (a `--after` the other way was refused as circular), so #1 was
        closed to release it; the pin is stormbootx's to do there.
- [x] #5 The stormnic contract: the driver brings its port up itself and
      reports one line per port (driver, PCI address, MAC, media, speed,
      FEC, link); the standard FEC for every e1000e media/speed (copper
      10/100/1000BASE-T) is none; a port without link is an error line.
      Same form as stormnic-igb#5 (b065f80).
  - [x] `e1000e::{Media, Fec, standard_fec, PortLine}`, binding's line, host tests (879f441)
  - [x] test-ovmf.sh checks the per-port line (FEC none, link UP)
  - [x] sc-build at 879f441: 41 host tests; check-driver 58,880 B subsystem 11;
        test-ovmf plain + ipxe PASS with `… media copper, speed 1000 Mb/s full
        duplex, FEC none, link UP`. Released v0.2.0; pin asked of stormbootx.
