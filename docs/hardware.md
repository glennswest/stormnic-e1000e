# How stormnic-e1000e drives the hardware

In this project's own words; the facts are from Intel's public datasheets
(82574 family; the PCH LAN controllers share the register set), and iPXE's
`intel.c` / Linux's `e1000e` were read for behaviour only.

## Parts

| Kind | Parts | NVM | MAC address |
|------|-------|-----|-------------|
| `M82574` | 82574L (8086:10d3, 10f6) | own EEPROM, EERD | RAL0/RAH0 (loaded at reset), else EEPROM words 0–2 |
| `Pch` | 82579, I217, I218, I219 (63 IDs) | a region of the platform SPI flash | RAL0/RAH0 (loaded at reset) |

All registers are 32-bit, in memory BAR 0.

## Registers used

| Offset | Name | Use |
|--------|------|-----|
| 0x0000 | CTRL | SLU+ASDE (link from the PHY), RST (MAC reset), GIO_MASTER_DISABLE; TFCE/RFCE kept clear |
| 0x0008 | STATUS | LU, FD, speed (bits 6–7), GIO_MASTER_EN, LAN_INIT_DONE (PCH) |
| 0x0014 | EERD | 82574 EEPROM read: word << 2 \| START, wait DONE, data in bits 16–31 |
| 0x0018 | CTRL_EXT | DRV_LOAD (bit 28): tells the management firmware a driver has the NIC |
| 0x0020 | MDIC | PHY access at MDIO address 1: reg << 16, phy << 21, op 26–27, READY 28, ERROR 30 |
| 0x0028–0x0030, 0x0170, 0x2160, 0x2168 | FCAL, FCAH, FCT, FCTTV, FCRTL, FCRTH | flow control: all zero |
| 0x00c0 / 0x00d8 | ICR / IMC | read to acknowledge / write all ones to mask: no interrupts |
| 0x0100 | RCTL | EN, UPE (promiscuous), MPE (all multicast), BAM, SECRC (strip CRC), 2 KiB buffers |
| 0x0400 / 0x0410 | TCTL / TIPG | EN, PSP, CT 0x0f, COLD 0x3f, RTLC / 0x00602008 |
| 0x0f00 | EXTCNF_CTRL | SWFLAG (bit 5): the PHY semaphore shared with the management firmware |
| 0x2800–0x2818 | RDBAL/H, RDLEN, RDH, RDT | receive ring |
| 0x3800–0x3818, 0x3828 | TDBAL/H, TDLEN, TDH, TDT, TXDCTL | transmit ring; TXDCTL GRAN, WTHRESH 1, PTHRESH 31 (+COUNT_DESC on the 82574) |
| 0x5200 | MTA[128] | cleared (MPE accepts every multicast; the SNP filters in software) |
| 0x5400 / 0x5404 | RAL0 / RAH0 | station address, AV bit 31 |

## Sequences

- **Reset** (`e1000e::reset`): GIO_MASTER_DISABLE and wait (≤ 10 ms) for
  GIO_MASTER_EN to clear; take the PHY semaphore; CTRL.RST; 20 ms with no
  access; wait (≤ 100 ms) for RST to clear; release the semaphore; on a PCH
  part wait (≤ 150 ms) for LAN_INIT_DONE and clear it; mask and acknowledge
  interrupts. **PHY_RST is never set**: the link the firmware brought up
  stays.
- **PHY check** (Start only): with the semaphore, read the PHY ID, BMCR and
  BMSR; if autonegotiation is off or the PHY is powered down or isolated,
  write BMCR with AUTOEN and RESTART_AN. A PHY that does not answer is a
  warning, not a failure.
- **Bring-up** (`ring::Net::start`, SNP Initialize): disable, reset, rings
  rewritten, then `mac_init` (CTRL with SLU/ASDE and no forced speed, no
  flow control; DRV_LOAD; RAL0/RAH0; MTA cleared; TXDCTL; ring registers
  with RDT = count − 1; TIPG, TCTL, RCTL) and last TCTL.EN, RCTL.EN.
- **Quiesce** (every exit): RX/TX off, interrupts masked, 10 ms, MAC reset,
  DRV_LOAD clear.

## Descriptors (legacy, 16 bytes)

Receive: buffer address (64 bits), length (16), checksum (16), status (8:
DD bit 0, EOP bit 1), errors (8: CE, SE, SEQ, CXE, RXE are fatal), special.
Transmit: buffer address, length (16), CSO (8), command (8: EOP, IFCS, RS),
status (8: DD), CSS, special.

The hardware owns receive descriptors [RDH, RDT) and transmit descriptors
[TDH, TDT). One receive descriptor is always held back; a consumed one is
returned by writing its index to RDT. Transmit never fills the last free
descriptor, so TDT never reaches TDH. The driver sees completion by DD.

## Not done (PCH parts, simulation only)

Linux applies several I217/I219 work-arounds this driver does not: the ULP
(ultra low power) exit on I219, the descriptor-ring flush before a reset on
I219 (SPT and later), K1/PLL settings, and the PHY workarounds after a PHY
reset (this driver never resets the PHY). On those machines the platform
firmware almost always carries Intel's own UEFI driver, which Supported
leaves in place; this driver binds only where a loader disconnects it.
