#!/bin/bash
#
# Report where our search first parts company with the C++ reference, for each
# position in manifest.tsv.
#
#   XRAY=~/src/bridge-solver-xray/xray/solver-xray \
#     fixtures/divergence/first-divergence.sh
#
# Build the reference's instrumented solver with:
#   clang++ -std=c++17 -O3 -o solver-xray xray/bridge-solver.cc
#
set -euo pipefail
cd "$(dirname "$0")/../.."

XRAY=${XRAY:?set XRAY to the built xray/solver-xray binary}
DIAG=${DIAG:-}
LIMIT=${LIMIT:-6000}

# Build solver-diag rather than trusting whatever is in target/. It needs
# `--features cli`, so the obvious build command fails and leaves the previous
# revision's binary in place -- and this script would then compare the
# reference against *that*, reporting `none` for a tree that has diverged.
# Silent success against stale code is the worst failure mode a divergence
# check can have, so the default path is always rebuilt.
if [ -z "$DIAG" ]; then
  DIAG=./target/release/solver-diag
  ./dev-build.sh --ci build --release --features cli --bin solver-diag >&2
else
  # An explicit DIAG is someone comparing a binary they built on purpose --
  # an older revision, say. Left alone, but it has to exist.
  [ -x "$DIAG" ] || { echo "DIAG=$DIAG is not an executable" >&2; exit 1; }
fi
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT

printf '%-10s %-18s %-10s %s\n' deal expected actual status
grep -v '^#' fixtures/divergence/manifest.tsv | while IFS=$'\t' read -r deal trump lead line iter cache mech; do
  [ -n "${deal:-}" ] || continue
  "$XRAY" -f "fixtures/divergence/$deal" -m0 -X "$LIMIT" 2>"$work/cxx" >/dev/null
  "$DIAG" -f "fixtures/divergence/$deal" -X "$LIMIT" 2>"$work/rs" >/dev/null
  # The two format `remaining=[...]` differently; it is diagnostic only.
  sed -E 's/[[:space:]]*remaining=\[[^]]*\]//' "$work/cxx" > "$work/cxx.n"
  sed -E 's/[[:space:]]*remaining=\[[^]]*\]//' "$work/rs"  > "$work/rs.n"
  got=$(cmp "$work/cxx.n" "$work/rs.n" 2>/dev/null | sed -E 's/.*line ([0-9]+).*/\1/') || true
  got=${got:-none}
  if [ "$got" = "$line" ]; then status=unchanged
  elif [ "$got" = none ]; then status="FIXED (no divergence)"
  else status="MOVED"; fi
  printf '%-10s %-18s %-10s %s\n' "$deal" "$line ($mech)" "$got" "$status"
done
