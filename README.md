# stormnic-e1000e

A Rust `no_std` UEFI Simple Network Protocol (SNP) driver for **Intel e1000e (82574L, 82579, I217–I219)** (8086:10D3, 1502, 153A, 15A0–15A3, 15B7+), loaded by
[stormbootx](https://github.com/glennswest/stormbootx) so it can claim and boot images over these NICs without iPXE.
A sibling of stormnic-ixgbe, stormnic-mlx4 and stormnic-virtio, built and pinned the same way (stormbootx `build-nic-drivers.sh`).

Status: not started (P3, paused; owner 2026-10-07).

## Licence

MIT (see LICENSE). NOTICE acknowledges the sources the hardware facts were learned from; no code was copied.
