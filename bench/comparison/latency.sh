#!/bin/bash
#
# Per-deal latency for all three solvers over one corpus. This is case 1 of
# METHODOLOGY.md -- one board, someone waiting.
#
#   bench/comparison/latency.sh CORPUSDIR REFBUILD [CHUNK] [OUT.tsv]
#
# The corpus is walked in chunks, and within each chunk every solver is run
# before moving on. That is the whole point of the chunking: this port and DDS
# are timed together inside one process, but the reference is a separate
# binary, and running it as a single pass after theirs would compare two
# solvers measured under whatever the machine was doing an hour apart. Machine
# drift is the largest error term in this measurement and interleaving is the
# only thing that cancels it.
#
# Output is one row per deal: ours, DDS and the reference in milliseconds, each
# the fastest of an adaptive number of repeats. See METHODOLOGY.md for why the
# repeat count is adaptive and why the minimum rather than the mean.
set -euo pipefail
cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"

CORPUS=${1:?usage: latency.sh CORPUSDIR REFBUILD [CHUNK] [OUT.tsv]}
REFBUILD=${2:?need the directory from build-reference.sh}
CHUNK=${3:-100}
OUT=${4:-latency.tsv}
BENCH=${BENCH:-./target/release/solver-bench}
RUNS=${RUNS:-15}
BUDGET=${BUDGET:-300}

[ -f "$CORPUS/deals.pbn" ]   || { echo "no deals.pbn in $CORPUS" >&2; exit 1; }
[ -f "$CORPUS/deals.hands" ] || { echo "no deals.hands in $CORPUS" >&2; exit 1; }
[ -x "$REFBUILD/solver-ref-latency" ] || { echo "no solver-ref-latency in $REFBUILD" >&2; exit 1; }
# The DDS column needs solver-bench built with the feature that links it, which
# is not the default and not what `--features bench` gives.
"$BENCH" latency --help 2>/dev/null | grep -q -- --dds || {
  echo "$BENCH has no --dds: rebuild it with" >&2
  echo "  ./dev-build.sh --ci build --release --features dds-reference --bin solver-bench" >&2
  exit 1; }

work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
NDEALS=$(grep -c . "$CORPUS/deals.pbn")
# The two formats are one line and four lines per deal respectively, so the
# chunk boundaries have to be cut at different line counts to line up.
split -l "$CHUNK"           -a 4 -d "$CORPUS/deals.pbn"   "$work/p."
split -l "$((CHUNK * 4))"   -a 4 -d "$CORPUS/deals.hands" "$work/h."

echo "deal,ours_ms,dds_ms,ref_ms" | tr ',' '\t' > "$OUT"
n=0
for p in "$work"/p.*; do
  h="$work/h.${p##*/p.}"
  [ -f "$h" ] || { echo "chunk mismatch: no $h" >&2; exit 1; }

  "$BENCH" latency "$p" --runs "$RUNS" --budget-ms "$BUDGET" --dds \
      --tsv "$work/ours.tsv" >/dev/null
  LATENCY_FILE="$h" LAT_RUNS="$RUNS" LAT_BUDGET_MS="$BUDGET" \
      "$REFBUILD/solver-ref-latency" > "$work/ref.tsv" 2>/dev/null

  # Both are numbered from 1 within the chunk; renumber onto the whole corpus.
  paste <(tail -n +2 "$work/ours.tsv") <(tail -n +2 "$work/ref.tsv") \
    | awk -v off="$n" 'BEGIN{OFS="\t"} {print off+$1, $2, $3, $5}' >> "$OUT"
  n=$((n + $(grep -c . "$p")))
  printf '\r%d/%d deals' "$n" "$NDEALS" >&2
done
echo >&2
echo "wrote $OUT ($((n)) deals)" >&2
