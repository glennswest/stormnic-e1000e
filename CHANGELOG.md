# Changelog

## [Unreleased]

### 2026-11-20
- **fix:** a MAC reset whose bus-master activity never drains
  (STATUS.GIO_MASTER_EN still set 10 ms after GIO_MASTER_DISABLE) fails
  with the new `Error::MasterTimeout` before CTRL.RST, instead of resetting
  the MAC under live DMA; start no longer rewrites the rings then (#9,
  findings 91bea4b60989 and 9680dd0bea37).
- **fix:** after a failed quiesce, cleanup never turns bus mastering back
  on: `decode::release_stopped` puts memory decode and the attributes back
  without BUS_MASTER and clears BME last, replacing `stop_bus_master` then
  the plain `release` (which restored BME when it was on before Start).
  Used on every keep-the-region path (Start's quiesce failure, a DMA region
  that could not be mapped, SNP install failure with the Port in use,
  binding Stop); ExitBootServices turns BME off if its quiesce fails (#9,
  finding cc9c5c400a6b).
- **test:** a simulator mode where GIO_MASTER_EN never clears; host tests
  for `decode` (new `test/decode.rs`, run by `scripts/test-host.sh`).

### 2026-11-14
- **docs:** refreshed README, docs/hardware.md and CLAUDE.md against the code since 2026-11-07 (the #5 per-port line, v0.2.0): the docs already match the code; added the open gaps #6 (older device IDs) and #7 (DPG_EN on PCH parts) to the README and work plan. No promise lacks code, so no new issue.

## [v0.2.0] — 2026-11-13

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
- **test:** verified at 879f441: 41 host tests pass; check-driver 58,880 B,
  PE32+ subsystem 11, clippy clean, no warnings; QEMU e1000e under OVMF
  (plain and iPXE takeover) prints `… media copper, speed 1000 Mb/s full
  duplex, FEC none, link UP` and passes DHCP, ping 5/5, UDP echo.
- **chore(release):** v0.2.0.
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
