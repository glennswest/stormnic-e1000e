# Changelog

## [Unreleased]

### 2026-11-13
- **feat:** the stormnic contract (#5): one console line per port naming
  driver, PCI address, PCI ID, part, MAC, media (copper), speed, the IEEE
  standard FEC for that media/speed (none: 10/100/1000BASE-T has no FEC)
  and link; a port without link is an `error:` line saying how long
  autonegotiation was waited for. `e1000e::{Media, Fec, standard_fec,
  PortLine}`; "SNP installed" and the flow-control state moved to the trace.
- **test:** host tests for the standard FEC and the per-port line (link UP,
  half duplex, the error line); `scripts/test-ovmf.sh` checks the per-port
  line with FEC none and link UP.
- **docs:** README and docs/hardware.md describe the contract and the line.

### 2026-11-07
- **docs:** refreshed README and docs/hardware.md from the code since 2026-10-07: the 800-series IDs (550c/550d, `Pch` kind) are named in both; no promise in the docs lacks code behind it, so no issue filed.
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
- **fix:** clippy and rustc findings from the first builds (#2, #3, #4): an
  enum variant renamed, a needless `mut`, a no-op transmute in the check app,
  an unused PciIo config write removed.
- **feat:** the 800-series chipset GbE IDs 8086:550c/550d (pci.ids), 67 IDs.
- **test:** verified: 38 host tests pass; check-driver 57,856 B, subsystem 11,
  clippy clean; QEMU e1000e under OVMF passes plain and iPXE-takeover boots.
- **docs:** #1 closed; the pin of d0bc5ec is asked on stormbootx#70.

### 2026-10-07
- **chore:** repository created (owner: low priority, paused)
