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
    """Two panels: the shape, and the difference.

    The absolute curve alone is nearly useless here -- three lines spanning
    three decades sit on top of each other and a 20% difference is invisible.
    But it is the shape worth seeing, because it is the whole argument for
    caring about the tail: flat and cheap until about p80, then an order of
    magnitude in the last few percent. So it goes on top, and underneath it the
    same curves divided by DDS, where the crossover is legible.
    """
    import math

    W, PH, M, GAP = 820, 250, 66, 54
    H = M + PH + GAP + PH + M
    n = len(srt[0])
    lo = max(1.0, min(s[0] for s in srt))
    hi = max(s[-1] for s in srt)
    colours = ["#2f6fb5", "#b5502f", "#4a8a4a", "#8a4a8a"]
    x = lambda i: M + (W - M - 130) * i / max(1, n - 1)
    top = lambda v: M + PH - PH * (math.log10(max(v, lo)) - math.log10(lo)) / (
        math.log10(hi) - math.log10(lo)
    )
    ratios = [[s[i] / srt[dds][i] for i in range(n)] for s in srt]
    # Scale off the bulk, not the extremes. The first percentile of deals runs
    # in a handful of milliseconds, where fixed per-solve overhead is most of
    # the time and the ratio swings wildly; letting that set the axis squashes
    # the region anyone cares about into a band. Points outside are clamped to
    # the edge rather than dropped, so the line still shows they went there.
    flat = sorted(v for k, r in enumerate(ratios) if k != dds for v in r)
    rlo = min(0.9, flat[int(0.01 * len(flat))])
    rhi = max(1.1, flat[int(0.99 * len(flat))])
    pad = 0.06 * (rhi - rlo)
    rlo, rhi = rlo - pad, rhi + pad
    ry = lambda v: M + PH + GAP + PH - PH * (min(max(v, rlo), rhi) - rlo) / (rhi - rlo)

    o = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" '
        f'font-family="system-ui,sans-serif" font-size="12">',
        f'<rect width="{W}" height="{H}" fill="#fff"/>',
        f'<text x="{M}" y="{M-34}" font-size="14" fill="#111">'
        f"Time to solve one board &#8212; {n} random deals, each solver sorted by its own time</text>",
    ]
    for dec in range(int(math.floor(math.log10(lo))), int(math.ceil(math.log10(hi))) + 1):
        v = 10.0**dec
        if lo <= v <= hi:
            o.append(
                f'<line x1="{M}" y1="{top(v):.1f}" x2="{W-130}" y2="{top(v):.1f}" stroke="#eee"/>'
                f'<text x="{M-8}" y="{top(v)+4:.1f}" text-anchor="end" fill="#777">{v:g} ms</text>'
            )
    for g in (1.0, rlo, rhi, (rlo + rhi) / 2, (rlo + 1.0) / 2, (rhi + 1.0) / 2):
        o.append(
            f'<line x1="{M}" y1="{ry(g):.1f}" x2="{W-130}" y2="{ry(g):.1f}" '
            f'stroke="{"#bbb" if abs(g-1)<1e-9 else "#eee"}"/>'
            f'<text x="{M-8}" y="{ry(g)+4:.1f}" text-anchor="end" fill="#777">{g:.2f}x</text>'
        )
    for p in (0, 50, 80, 90, 95, 100):
        i = min(n - 1, int(p / 100 * n))
        for y0, y1 in ((M, M + PH), (M + PH + GAP, M + PH + GAP + PH)):
            o.append(f'<line x1="{x(i):.1f}" y1="{y0}" x2="{x(i):.1f}" y2="{y1}" stroke="#f4f4f4"/>')
        o.append(
            f'<text x="{x(i):.1f}" y="{H-M+18}" text-anchor="middle" fill="#777">p{p}</text>'
        )
    for k, s in enumerate(srt):
        o.append(
            '<polyline points="'
            + " ".join(f"{x(i):.1f},{top(v):.1f}" for i, v in enumerate(s))
            + f'" fill="none" stroke="{colours[k%4]}" stroke-width="1.8"/>'
        )
        if k != dds:
            o.append(
                '<polyline points="'
                + " ".join(f"{x(i):.1f},{ry(v):.1f}" for i, v in enumerate(ratios[k]))
                + f'" fill="none" stroke="{colours[k%4]}" stroke-width="1.8"/>'
            )
    o.append(
        f'<text x="{M}" y="{M+PH+GAP-16}" font-size="13" fill="#111">'
        f"Ratio to {label[dds]} at the same percentile &#8212; below 1.00 is faster</text>"
    )
    for k, lab in enumerate(label):
        yy = M + 6 + k * 18
        o.append(
            f'<line x1="{W-118}" y1="{yy-4}" x2="{W-98}" y2="{yy-4}" '
            f'stroke="{colours[k%4]}" stroke-width="3"/>'
            f'<text x="{W-92}" y="{yy}" fill="#333">{lab}</text>'
        )
    o.append("</svg>")
    open(path, "w").write("\n".join(o))


if __name__ == "__main__":
    main()
