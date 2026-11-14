#!/usr/bin/env bash
# The driver under OVMF in QEMU, on the build box (needs no root):
#
#   sc-build scripts/test-ovmf.sh
#
# QEMU's `e1000e` device is an Intel 82574L (8086:10d3). Builds the check
# image (scripts/build-image.sh) with a UDP echo port for slirp, then boots
# it twice:
#
#   plain  romfile= : no firmware driver holds the NIC; the check app loads
#                     ours and connects it
#   ipxe   QEMU's efi-e1000e.rom (iPXE): the firmware's driver binds the NIC
#                     first and OVMF stacks its network boot on it; the check
#                     app has to take the NIC from it (as stormbootx's
#                     `prefer_media_drivers` does)
#
# slirp leases 10.0.2.15 and answers pings to its 10.0.2.2; the UDP echo
# goes to a python echo server on the build box's loopback, which slirp
# reaches as 10.0.2.2. Each boot must print `STORMNIC-E1000E CHECK PASS`
# and the driver's per-port line (FEC none, link UP).
#
# Needs qemu-system-x86_64, OVMF, python3, mkfs.fat, mtools. KVM when
# /dev/kvm is writable, TCG otherwise.
set -euo pipefail
OVMF_CODE=${OVMF_CODE:-/usr/share/edk2/ovmf/OVMF_CODE.fd}
OVMF_VARS=${OVMF_VARS:-/usr/share/edk2/ovmf/OVMF_VARS.fd}
LIMIT=${LIMIT:-240}
say() { printf 'test-ovmf: %s\n' "$*"; }
die() { say "FAIL: $*"; exit 1; }
command -v qemu-system-x86_64 >/dev/null || die "qemu-system-x86_64 is not installed"
[[ -r "$OVMF_CODE" ]] || die "no OVMF at $OVMF_CODE"
qemu-system-x86_64 -device help 2>/dev/null | grep -q '"e1000e"' || die "this QEMU has no e1000e device"

W=$(mktemp -d "${TMPDIR:-/tmp}/test-ovmf.XXXXXX")
ECHO_PID=""
cleanup() { [[ -n "$ECHO_PID" ]] && kill "$ECHO_PID" 2>/dev/null; rm -rf "$W"; }
trap cleanup EXIT

# A UDP echo server; port 0 lets the kernel pick, written down for the image.
python3 - "$W/echo.port" <<'PY' &
import socket, sys
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.bind(("127.0.0.1", 0))
open(sys.argv[1], "w").write(str(s.getsockname()[1]))
while True:
    data, peer = s.recvfrom(65536)
    s.sendto(data, peer)
PY
ECHO_PID=$!
for _ in $(seq 50); do [[ -s "$W/echo.port" ]] && break; sleep 0.1; done
PORT=$(cat "$W/echo.port") || die "the UDP echo server did not start"
say "UDP echo on 127.0.0.1:$PORT (10.0.2.2:$PORT from the guest)"

CHECK_CONF="udp_port=$PORT" scripts/build-image.sh "$W/check.img"

accel=tcg
[[ -w /dev/kvm ]] && accel=kvm
# boot NAME ROMFILE TAKEOVER(yes|no)
boot() {
    local name=$1 rom=$2 takeover=$3 log="$W/$1.log"
    cp "$OVMF_VARS" "$W/vars.fd"
    say "boot $name (romfile=${rom:-none}, $accel)"
    timeout "$LIMIT" qemu-system-x86_64 -machine q35,accel="$accel" -m 512 -no-reboot \
        -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
        -drive if=pflash,format=raw,file="$W/vars.fd" \
        -drive file="$W/check.img",format=raw,if=none,id=disk -device virtio-blk-pci,drive=disk,bootindex=1 \
        -netdev user,id=n0 -device e1000e,netdev=n0,romfile="$rom" \
        -serial file:"$log" -monitor none -display none </dev/null >/dev/null 2>&1 || true
    sed -e 's/\x1b\[[0-9;]*[A-Za-z]//g' "$log" | tr -d '\r' | grep -E '^(stormnic-e1000e|check:|STORMNIC-E1000E)' || true
    grep -q 'STORMNIC-E1000E CHECK PASS' "$log" || die "$name: no PASS line"
    if [[ "$takeover" == yes ]]; then
        grep -q 'firmware driver(s) disconnected' "$log" \
            || die "$name: the firmware's driver did not hold the NIC, so the takeover was not tested"
    fi
    # The per-port line of the stormnic contract (#5): address, MAC, media,
    # speed, the standard FEC (none at 1G) and link UP.
    grep -qE "stormnic-e1000e [0-9.]+: [0-9a-f:.]+ 8086:10d3 [^:]+: MAC ([0-9a-f]{2}:){5}[0-9a-f]{2}, media copper, speed 1000 Mb/s full duplex, FEC none, link UP" "$log" \
        || die "$name: no per-port line for 8086:10d3 with FEC none and link UP"
    say "$name: PASS"
}
boot plain "" no
ROM=$(ls /usr/share/qemu/efi-e1000e.rom /usr/share/ipxe/qemu/efi-e1000e.rom 2>/dev/null | head -1 || true)
if [[ -n "$ROM" ]]; then
    boot ipxe "$ROM" yes
else
    say "no efi-e1000e.rom on this box: the iPXE takeover boot is skipped"
fi
say "all boots passed"
