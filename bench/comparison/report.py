#!/usr/bin/env python3
"""Turn a latency.sh TSV into the case-1 table and percentile plot.

    ./report.py latency.tsv [plot.svg]

Percentiles rather than a mean, because deal cost is skewed enough that a mean
is a tail measurement in disguise -- the slowest fifth of random deals is about
half of all the work. DDS is 1.00 in every ratio, being the one everybody
already has.
"""
import sys


def pct(sorted_vals, p):
    """Nearest-rank percentile."""
    if not sorted_vals:
        return 0.0
    i = max(0, min(len(sorted_vals) - 1, int((p / 100.0) * len(sorted_vals) + 0.5) - 1))
    return sorted_vals[i]


def read(path):
    cols, rows = None, []
    for line in open(path):
        parts = line.rstrip("\n").split("\t")
        if cols is None:
            cols = parts
            continue
        if len(parts) < len(cols):
            continue
        rows.append([float(x) if x else float("nan") for x in parts[1:]])
    names = cols[1:]
    return names, [[r[i] for r in rows] for i in range(len(names))]


PRETTY = {"ours_ms": "this port", "dds_ms": "DDS 2.9", "ref_ms": "C++ reference"}


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    names, series = read(sys.argv[1])
    label = [PRETTY.get(n, n) for n in names]
    dds = names.index("dds_ms") if "dds_ms" in names else 0
    n = len(series[0])
    srt = [sorted(s) for s in series]

    print(f"{n} random deals, full 20-cell table each, single-threaded")
    print(f"milliseconds, fastest of an adaptive number of repeats\n")
    head = f"{'':>6}" + "".join(f"{x:>16}" for x in label)
    print(head)
    print("-" * len(head))
    for p in (50, 80, 90, 95, 99, 100):
        tag = "max" if p == 100 else f"p{p}"
        cells = "".join(
            f"{pct(s, p):>10.1f}{'':1}{pct(s, p) / pct(srt[dds], p):>5.2f}x" for s in srt
        )
        print(f"{tag:>6}{cells}")
    print()
    tot = [sum(s) for s in series]
    print(f"{'total':>6}" + "".join(f"{t/1000:>10.1f}s{t/tot[dds]:>5.2f}x" for t in tot))
    print(f"\n(each cell: milliseconds, then the ratio to {label[dds]})")

    # The shape is the point: flat and indistinguishable until the tail, then a
    # steep climb where the three separate. A log y-axis is what makes both
    # ends legible at once.
    if len(sys.argv) > 2:
        write_plot(sys.argv[2], label, srt, dds)
        print(f"\nwrote {sys.argv[2]}")


def write_plot(path, label, srt, dds):
    import math

    W, H, M = 760, 420, 58
    n = len(srt[0])
    lo = max(1.0, min(s[0] for s in srt))
    hi = max(s[-1] for s in srt)
    x = lambda i: M + (W - 2 * M) * i / max(1, n - 1)
    y = lambda v: H - M - (H - 2 * M) * (math.log10(max(v, lo)) - math.log10(lo)) / (
        math.log10(hi) - math.log10(lo)
    )
    colours = ["#2f6fb5", "#b5502f", "#4a8a4a", "#8a4a8a"]
    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" '
        f'font-family="system-ui,sans-serif" font-size="12">',
        f'<rect width="{W}" height="{H}" fill="#fff"/>',
    ]
    for dec in range(int(math.floor(math.log10(lo))), int(math.ceil(math.log10(hi))) + 1):
        v = 10.0**dec
        if not (lo <= v <= hi):
            continue
        out.append(
            f'<line x1="{M}" y1="{y(v):.1f}" x2="{W-M}" y2="{y(v):.1f}" stroke="#e4e4e4"/>'
            f'<text x="{M-8}" y="{y(v)+4:.1f}" text-anchor="end" fill="#666">{v:g} ms</text>'
        )
    for p in (0, 50, 80, 90, 95, 100):
        i = min(n - 1, int(p / 100 * n))
        out.append(
            f'<line x1="{x(i):.1f}" y1="{M}" x2="{x(i):.1f}" y2="{H-M}" stroke="#f0f0f0"/>'
            f'<text x="{x(i):.1f}" y="{H-M+18}" text-anchor="middle" fill="#666">p{p}</text>'
        )
    for k, s in enumerate(srt):
        pts = " ".join(f"{x(i):.1f},{y(v):.1f}" for i, v in enumerate(s))
        out.append(
            f'<polyline points="{pts}" fill="none" stroke="{colours[k%4]}" stroke-width="2"/>'
        )
        out.append(
            f'<text x="{W-M+4}" y="{y(s[-1])+4:.1f}" fill="{colours[k%4]}">{label[k]}</text>'
        )
    out.append(
        f'<text x="{M}" y="{M-18}" font-size="13" fill="#222">'
        f"Time to solve one board, {n} random deals, sorted</text>"
    )
    out.append("</svg>")
    open(path, "w").write("\n".join(out))


if __name__ == "__main__":
    main()
