#!/bin/bash
#
# Build the C++ reference for the comparison: one plain binary, and one
# instrumented for per-deal latency. Both with PGO, which is what its own
# makefile's default target does.
#
#   bench/comparison/build-reference.sh <path to macroxue/bridge-solver> [OUTDIR]
#
# Its makefile drives PGO the GCC way -- `-fprofile-generate`, then
# `mv solver.p-solver.gcda solver.gcda`, then `-fprofile-use` -- which does not
# work with clang. The clang equivalents below train on the same deal
# (`hard_deals/deal.8`) so the profile is the one upstream's build would use.
set -euo pipefail

SRC=${1:?usage: build-reference.sh <path to macroxue/bridge-solver checkout> [outdir]}
OUT=${2:-$(pwd)/ref-build}
[ -f "$SRC/solver.cc" ] || { echo "no solver.cc in $SRC" >&2; exit 1; }
PATCH="$(cd "$(dirname "$0")" && pwd)/ref-latency.patch"

mkdir -p "$OUT"
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
cp "$SRC/solver.cc" "$work/plain.cc"
cp "$SRC/solver.cc" "$work/latency.cc"
patch -s -p1 "$work/latency.cc" < "$PATCH"

build_pgo() {  # build_pgo <source> <output name>
  # Separate `local` statements: bash declares every name in one `local` before
  # assigning any of them, so referring to `$name` in the same statement reads
  # an unset variable, which `set -u` then makes fatal.
  local src=$1
  local name=$2
  local prof="$work/prof-$name"
  rm -rf "$prof"; mkdir -p "$prof"
  clang++ -std=c++17 -O3 -fprofile-generate="$prof" -o "$work/$name.pgen" "$src"
  # The training run its makefile uses. Output discarded; we want the profile.
  ( cd "$SRC" && "$work/$name.pgen" -i -f hard_deals/deal.8 >/dev/null 2>&1 ) || true
  xcrun llvm-profdata merge -output="$prof/merged.profdata" "$prof"/*.profraw
  clang++ -std=c++17 -O3 -fprofile-use="$prof/merged.profdata" -o "$OUT/$name" "$src"
  echo "built $OUT/$name"
}

build_pgo "$work/plain.cc"   solver-ref
build_pgo "$work/latency.cc" solver-ref-latency

cat <<EOF

Done.
  $OUT/solver-ref          one deal per invocation, as upstream ships it
  $OUT/solver-ref-latency  adds one env-var mode, no algorithmic change:
                             LATENCY_FILE=<hands file>   solve every deal in it
                             LAT_RUNS / LAT_BUDGET_MS    repeat rule (see below)
                           Default is adaptive best-of for latency work.
                           LAT_RUNS=1 LAT_BUDGET_MS=0 makes it a single pass
                           over the file, which is the bulk-throughput mode.
EOF
