#!/usr/bin/env bash
# Build the driver and check its image is what the firmware needs: a PE32+
# EFI boot-service driver (subsystem 11) for x86_64, not an application
# (10), which the firmware would unload as soon as its entry point returned.
# Clippy runs on the driver and the check app with warnings as errors.
#
#   sc-build scripts/check-driver.sh
#
# SC_BUILD_OUT can collect dist/: the .efi and its sha256.
set -euo pipefail
cd "$(dirname "$0")/.."
t=x86_64-unknown-uefi
target_dir="${CARGO_TARGET_DIR:-target}"
mkdir -p dist
# --locked: Cargo.lock is committed; a build must not re-resolve deps.
cargo build --locked --release --target "$t"
cargo clippy --locked --release --target "$t" -- -D warnings
cargo clippy --locked --release --target "$t" -p stormnic-e1000e-check -- -D warnings
cp "$target_dir/$t/release/stormnic-e1000e.efi" dist/stormnic-e1000e.efi

python3 - dist/stormnic-e1000e.efi <<'PY'
import hashlib, struct, sys
names = {10: "EFI_APPLICATION", 11: "EFI_BOOT_SERVICE_DRIVER", 12: "EFI_RUNTIME_DRIVER"}
path = sys.argv[1]
data = open(path, "rb").read()
pe = struct.unpack_from("<I", data, 0x3c)[0]
assert data[pe:pe + 4] == b"PE\0\0", "not a PE image"
machine = struct.unpack_from("<H", data, pe + 4)[0]
opt = pe + 24
magic = struct.unpack_from("<H", data, opt)[0]
subsystem = struct.unpack_from("<H", data, opt + 68)[0]
print(f"{path}: {len(data)} bytes, sha256 {hashlib.sha256(data).hexdigest()}")
print(f"  machine 0x{machine:x}, magic 0x{magic:x} (PE32+ is 0x20b), subsystem {subsystem} ({names.get(subsystem, '?')})")
assert machine == 0x8664 and magic == 0x20b, "not a PE32+ x86_64 image"
assert subsystem == 11, "not linked as an EFI boot-service driver"
print("  OK: EFI boot-service driver")
PY
( cd dist && sha256sum stormnic-e1000e.efi > SHA256SUMS )
