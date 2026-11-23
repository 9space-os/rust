#!/bin/bash
# Build cargo (rust/src/tools/cargo) for x86_64-unknown-plan9 with the stage-1 rustc, the
# crates of crates/ (cargo-plan9.toml) and the cross gcc of toolchain/gcc (GCC_DIR in
# paths.env; GCC9, GCC9BUILD override) for the C in it (APE).
# Result: tests/out/cargo-port/x86_64-unknown-plan9/release/cargo; log: logs/cargo-port.log.
# objtype=386: for i686-unknown-plan9 (tests/out/cargo-port/i686-unknown-plan9/release/cargo,
# logs/cargo-port.386.log); objtype=arm64 likewise for aarch64-unknown-plan9, objtype=riscv64 for
# riscv64gc-unknown-plan9.
TOP=$(cd "$(dirname "$0")/.." && pwd)
. "$TOP/scripts/9space-paths.sh"	# TREE, TREE_GCC, GCC_DIR, ... (the distribution's paths.env)
mkdir -p "$TOP/logs"	# not in git (.gitignore): a fresh checkout has none
GCC9=${GCC9:-$GCC_DIR}
# its build directory with the toolchain over gcc's APE (9front branch gcc)
GCC9BUILD=${GCC9BUILD:-$GCC9/build}
case ${objtype:=amd64} in
amd64)	T=x86_64-unknown-plan9 X=x86_64-pc-plan9 LOG=cargo-port.log ;;
386)	T=i686-unknown-plan9 X=i686-pc-plan9 LOG=cargo-port.386.log ;;
arm64)	T=aarch64-unknown-plan9 X=aarch64-pc-plan9 LOG=cargo-port.arm64.log ;;
riscv64)	T=riscv64gc-unknown-plan9 X=riscv64-unknown-plan9 LOG=cargo-port.riscv64.log ;;
*)	echo "build-cargo.sh: objtype=$objtype: no target" >&2; exit 1 ;;
esac
TV=$(echo $T | tr a-z- A-Z_)
export PATH=$GCC9BUILD/$objtype/cross/bin:$PATH
export RUSTC=$TOP/rust/build/host/stage1/bin/rustc
export CARGO_TARGET_DIR=$TOP/tests/out/cargo-port
# cargo keeps crates built against an older stage-1 std (rustc's version string does
# not change with it: "found possibly newer version of crate core"): the output starts
# anew when the stds it was built with (the targets', and the host's for build scripts
# and proc macros) are not the ones there now
core=$(ls -l --time-style=+%s "$TOP"/rust/build/host/stage1/lib/rustlib/*/lib/libcore-*.rlib 2>/dev/null | awk '{print $6, $7}')
stamp=$CARGO_TARGET_DIR.std
if [ -n "$core" ] && [ "$(cat "$stamp" 2>/dev/null)" != "$core" ]; then
	rm -rf "$CARGO_TARGET_DIR"
	echo "$core" > "$stamp"
fi
export CC_${T//-/_}=$X-gcc CXX_${T//-/_}=$X-g++
# the C of crates: APE programs as any (the target links APE beside libc's libp9.a)
export AR_${T//-/_}=$X-ar
# libz-sys: its own zlib (APE's is old)
export LIBZ_SYS_STATIC=1
export CARGO_TARGET_${TV}_LINKER=$X-gcc
cargo build --keep-going --release --target $T \
	--manifest-path "$TOP/rust/src/tools/cargo/Cargo.toml" \
	--config "$TOP/cargo-plan9.toml" "$@" > "$TOP/logs/$LOG" 2>&1
r=$?
grep -E '^error' "$TOP/logs/$LOG" | sort | uniq -c | head -40
exit $r
