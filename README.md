# stormnic-e1000e

A Rust `no_std` UEFI Simple Network Protocol (SNP) driver for the **Intel
e1000e** gigabit family — the discrete **82574L** (8086:10d3, 10f6) and the
chipset-integrated **82579, I217, I218 and I219** (67 device IDs in
`src/ids.rs`, checked against pci.ids 2026.11.06) — loaded by
[stormbootx](https://github.com/glennswest/stormbootx) so it can claim and
boot images over these NICs without iPXE. A sibling of stormnic-ixgbe,
stormnic-mlx4, stormnic-virtio and stormnic-realtek, built and pinned the
same way (stormbootx `build-nic-drivers.sh`).

Status: v0.1.0, #1. The 82574L passes under QEMU/OVMF (QEMU's `e1000e`
device, `scripts/test-ovmf.sh`, 2026-11-07 at e5384c7: DHCP, ping 5/5, UDP
echo, with and without QEMU's iPXE option ROM holding the NIC first). The
PCH parts (82579, I217–I219) are verified in simulation only, and their
console line says so. Not yet pinned in stormbootx (stormbootx#70).

## What it does

- `EFI_DRIVER_BINDING_PROTOCOL` on the image (an EFI boot-service driver,
  PE subsystem 11). Supported declines a function another driver already
  holds BY_DRIVER, so the platform's own driver (Intel's UNDI, QEMU's iPXE
  option ROM) wins unless a loader disconnects it first.
- Start: memory decode and bus mastering on, the station address from
  receive address 0 (or the 82574's EEPROM), the PHY made to autonegotiate
  (never reset), one bring-up with a broadcast DMA check when the link is up,
  then the NIC quiesced and an `EFI_SIMPLE_NETWORK_PROTOCOL` with a MAC
  device path installed on a child handle.
- SNP: polled legacy descriptor rings (32 RX + 32 TX, 2 KiB buffers),
  Start/Stop/Initialize/Reset/Shutdown, ReceiveFilters (unicast, broadcast,
  multicast list, promiscuous), GetStatus with buffer recycling,
  Transmit/Receive, WaitForPacket, MCastIpToMac. Statistics and NvData are
  unsupported; the station address is the NIC's own.
- **Flow control off**: the MAC never sends pause frames.
- The NIC runs only between SNP Initialize and Shutdown/Stop; it is quiesced
  (RX/TX off, interrupts masked, MAC reset, CTRL_EXT.DRV_LOAD clear) at
  every other point, including ExitBootServices.
- Console: one line per NIC by default
  (`stormnic-e1000e 0.1.0: 00:02.0 8086:10d3 82574L Gigabit Network Connection: MAC …, link up 1000 Mb/s full duplex, flow control off, SNP installed`);
  the bring-up trace when the EFI variable `StormnicVerbose`
  (GUID `ce1479a2-eab9-4176-b0ad-c909ea5b8e0b`) is set or with
  `--features verbose`.

How the hardware is driven: [docs/hardware.md](docs/hardware.md).

## Build and test

Never on the session VM: push, then

```bash
sc-build scripts/check-driver.sh   # build, clippy -D warnings, PE32+ subsystem 11 check
sc-build scripts/test-host.sh      # simulated-controller tests (test/)
sc-build scripts/test-ovmf.sh      # QEMU e1000e under OVMF: DHCP, ping, UDP echo
```

`scripts/build-image.sh` lays the driver and the check app (`check/`) on a
bootable GPT/FAT disk image (`\EFI\BOOT\BOOTX64.EFI`,
`\stormboot\drivers\stormnic-e1000e.efi`).

## Licence

MIT (see LICENSE). NOTICE acknowledges the sources the hardware facts were
learned from; no code was copied.
