# scripts/9space-paths.sh: sourced after TOP (this repository) is set:
# the distribution's paths, by dist/lib/9space-paths.sh (dist beside this
# repository or one level up: ../dist, ../../dist for toolchain/*)
for __d in "$TOP/../dist" "$TOP/../../dist"; do [ -f "$__d/lib/9space-paths.sh" ] && break; __d=; done
[ -n "$__d" ] || { echo "9space-paths: no dist/ beside $TOP (dist/bin/repos)" >&2; exit 1; }
. "$__d/lib/9space-paths.sh" || exit 1
unset __d
