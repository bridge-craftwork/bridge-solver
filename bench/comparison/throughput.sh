#!/bin/bash
#
# Wall clock for a whole set of deals, on all three solvers, at one or more
# thread counts. This is cases 2 and 3 of METHODOLOGY.md -- an event someone is
# waiting for, and a file nobody is watching.
#
#   bench/comparison/throughput.sh CORPUSDIR REFBUILD "1 4 8 12" [ROUNDS]
#
# CORPUSDIR is a directory from gen-corpus.py. REFBUILD is the directory from
# build-reference.sh.
#
# Each solver is given the machine the way it can actually use it. DDS and this
# port take a thread count. The C++ reference has no threads and is not
# thread-safe -- its caches and stats are global mutable state -- so its only
# route to more than one core is more than one process, which is what its own
# parallel_run_tests.sh does with `xargs -P`. Comparing threads against
# processes is the point rather than a flaw: what a user gets is deals per
# second from the machine, however the solver arranges it.
set -euo pipefail
cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"

CORPUS=${1:?usage: throughput.sh CORPUSDIR REFBUILD "THREADS..." [ROUNDS]}
REFBUILD=${2:?need the directory from build-reference.sh}
THREADS=${3:-"1 4 8 12"}
ROUNDS=${4:-3}
BENCH=${BENCH:-./target/release/solver-bench}

[ -f "$CORPUS/deals.pbn" ] || { echo "no deals.pbn in $CORPUS" >&2; exit 1; }
[ -x "$REFBUILD/solver-ref" ] || { echo "no solver-ref in $REFBUILD" >&2; exit 1; }
NDEALS=$(grep -c . "$CORPUS/deals.pbn")

secs() { python3 -c 'import time;print(f"{time.time():.3f}")'; }
best() { sort -g | head -1; }

printf '%s deals, best of %s rounds\n\n' "$NDEALS" "$ROUNDS"
printf '%-8s %10s %10s %10s   %s\n' threads ours dds ref "(deals/sec)"

for t in $THREADS; do
  ours_b=""; dds_b=""; ref_b=""
  for _ in $(seq 1 "$ROUNDS"); do
    # One process per thread count: DDS cannot survive a second SetResources.
    out=$("$BENCH" reference --throughput-pbn "$CORPUS/deals.pbn" --runs 1 \
            --dds-threads "$t" --dds-threading 5 2>/dev/null)
    ours_b+="$(echo "$out" | awk '$1=="ours"{print $2}')"$'\n'
    dds_b+="$(echo "$out" | awk '$1=="dds"{print $2}')"$'\n'

    # The reference: one process per deal, fanned out N at a time.
    s=$(secs)
    ls "$CORPUS/deals" | xargs -I{} -P "$t" "$REFBUILD/solver-ref" \
        -f "$CORPUS/deals/{}" >/dev/null 2>&1
    e=$(secs)
    ref_b+="$(python3 -c "print(f'{($e-$s)*1000:.1f}')")"$'\n'
  done
  o=$(echo "$ours_b" | grep . | best)
  d=$(echo "$dds_b"  | grep . | best)
  r=$(echo "$ref_b"  | grep . | best)
  python3 - "$t" "$o" "$d" "$r" "$NDEALS" <<'PY'
import sys
t, o, d, r, n = sys.argv[1], *map(float, sys.argv[2:5]), int(sys.argv[5])
print(f"{t:<8} {n/(o/1000):10.1f} {n/(d/1000):10.1f} {n/(r/1000):10.1f}"
      f"   ours {o/d:.3f}x dds, ref {r/d:.3f}x dds")
PY
done
