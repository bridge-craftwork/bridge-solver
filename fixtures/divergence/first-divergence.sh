#!/bin/bash
#
# Report where our search first parts company with the C++ reference, for each
# position in manifest.tsv.
#
# The deals are named rather than copied: they live in macroxue/bridge-solver's
# `1k_deals`, which is GPL-2.0, and this repo is MIT OR Apache-2.0. Point
# UPSTREAM at a checkout.
#
#   UPSTREAM=~/src/bridge-solver XRAY=~/src/bridge-solver-xray/xray/solver-xray \
#     fixtures/divergence/first-divergence.sh
#
# Build the reference's instrumented solver with:
#   clang++ -std=c++17 -O3 -o solver-xray xray/bridge-solver.cc
#
set -euo pipefail
cd "$(dirname "$0")/../.."

UPSTREAM=${UPSTREAM:?set UPSTREAM to a macroxue/bridge-solver checkout}
XRAY=${XRAY:?set XRAY to the built xray/solver-xray binary}
DIAG=${DIAG:-./target/release/solver-diag}
LIMIT=${LIMIT:-6000}
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT

printf '%-10s %-18s %-10s %s\n' deal expected actual status
grep -v '^#' fixtures/divergence/manifest.tsv | while IFS=$'\t' read -r deal trump lead line mech; do
  [ -n "${deal:-}" ] || continue
  { grep -v '^[[:space:]]*$' "$UPSTREAM/1k_deals/$deal"; echo "$trump"; echo "$lead"; } > "$work/d"
  "$XRAY" -f "$work/d" -m0 -X "$LIMIT" 2>"$work/cxx" >/dev/null
  "$DIAG" -f "$work/d" -X "$LIMIT" 2>"$work/rs" >/dev/null
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
