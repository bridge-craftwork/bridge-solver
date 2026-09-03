#!/usr/bin/env python3
"""Generate the shared comparison corpus, in both solvers' input formats.

Seeded, so the corpus is reproducible from this script rather than committed
as data. Random deals, because that is the workload the published comparisons
use and the one a user's own deals resemble; the curated `bench/corpus.json` is
for tracking this port against itself, not for comparing products.

    ./gen-corpus.py OUTDIR [COUNT] [SEED]

writes OUTDIR/deals.pbn        one PBN deal string per line, for solver-bench
       OUTDIR/deals.hands      four lines per deal, for the C++ reference
       OUTDIR/deals/deal.N     one file per deal, which is the only shape the
                               reference's own `xargs -P` fan-out can consume
"""
import os
import random
import sys

RANKS = "AKQJT98765432"
NUM_SUITS = 4


def deal_one(rng):
    """One shuffled deal as four 13-card hands, in PBN's N-E-S-W order."""
    cards = [(s, r) for s in range(NUM_SUITS) for r in range(13)]
    rng.shuffle(cards)
    return [cards[i * 13:(i + 1) * 13] for i in range(4)]


def suits(hand):
    """A hand as four suit strings, high card first, a void as '-'."""
    out = []
    for suit in range(NUM_SUITS):
        ranks = sorted(r for (s, r) in hand if s == suit)
        out.append("".join(RANKS[r] for r in ranks) if ranks else "-")
    return out


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    outdir = sys.argv[1]
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 2000
    seed = int(sys.argv[3]) if len(sys.argv) > 3 else 20260902
    os.makedirs(outdir, exist_ok=True)
    rng = random.Random(seed)

    pbn, hands_fmt = [], []
    for _ in range(count):
        north, east, south, west = (suits(h) for h in deal_one(rng))
        pbn.append("N:" + " ".join(".".join(h) for h in (north, east, south, west)))
        # The reference reads North, West, East, South, one hand per line, and
        # splits East off West's line if it finds four spaces there -- so each
        # hand goes on its own line and the ambiguity never arises.
        hands_fmt.append("\n".join(" ".join(h) for h in (north, west, east, south)))

    with open(os.path.join(outdir, "deals.pbn"), "w") as f:
        f.write("\n".join(pbn) + "\n")
    with open(os.path.join(outdir, "deals.hands"), "w") as f:
        f.write("\n".join(hands_fmt) + "\n")

    # The reference takes one deal per invocation and has no multi-deal input,
    # so using more than one core with it means one process per deal file --
    # which is what its own parallel_run_tests.sh does.
    perdeal = os.path.join(outdir, "deals")
    os.makedirs(perdeal, exist_ok=True)
    for i, block in enumerate(hands_fmt, 1):
        with open(os.path.join(perdeal, f"deal.{i}"), "w") as f:
            f.write(block + "\n")

    print(f"{count} deals, seed {seed} -> {outdir}/"
          f"{{deals.pbn, deals.hands, deals/deal.1..{count}}}")


if __name__ == "__main__":
    main()
