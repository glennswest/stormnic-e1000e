# Changelog

## [Unreleased]

### 2026-11-07
- **feat:** the e1000e SNP driver (#1): driver binding for 65 Intel
  82574L/82579/I217/I218/I219 device IDs (pci.ids 2026.11.06), MAC reset
  without a PHY reset, station address from RAL0 or the 82574 EEPROM, PHY
  autonegotiation check over MDIC with the firmware semaphore, polled legacy
  descriptor rings, the SNP state machine from stormnic-realtek, flow control
  off, the NIC quiesced at every exit and at ExitBootServices, one console
  line per NIC with the trace behind `StormnicVerbose`.
- **test:** host tests against a simulated controller (`test/`,
  `scripts/test-host.sh`); `scripts/check-driver.sh` (build, clippy -D
  warnings, PE32+ subsystem 11); the check app and `scripts/test-ovmf.sh`
  (QEMU e1000e under OVMF: DHCP, ping, UDP echo, with and without QEMU's iPXE
  option ROM holding the NIC).
- **docs:** README, docs/hardware.md, NOTICE, CLAUDE.md work plan.

### 2026-10-07
- **chore:** repository created (owner: low priority, paused)
