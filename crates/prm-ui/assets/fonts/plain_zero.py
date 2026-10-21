"""Give the bundled Atkinson Hyperlegible fonts a plain zero (an oval with no slash).

Why: the upstream `zero` glyph in both Atkinson Hyperlegible Next and Mono has a diagonal
slash. Fonts usually offer an unslashed zero through an OpenType feature, but egui cannot
apply OpenType features and neither font has such an alternate. So this script edits the
default `zero` glyph itself.

What it does, for each font:

- `zero` has three contours: the outer oval (contour 0) and the two halves of the counter
  that the slash splits apart (contours 1 and 2). The two halves are replaced by one inner
  contour: the outer contour, scaled about its centre and reversed in direction so that it
  cuts a counter.
- The scale is chosen so the new counter has the same box as the two old halves together,
  which keeps the stroke weights of the design. This is done separately for the default
  instance and for each `gvar` variation (at its peak), so the counter also matches the
  original one at the lightest and boldest weights.
- The outer contour, its variation deltas, the phantom point deltas (advance widths) and
  `hmtx` are left as they are. Glyphs that reference `zero` (such as `zero.tf` in Next) pick
  up the change. Mono's `zero.zero` (the `zero` feature alternate) is not touched.

Usage, from this directory:

    python plain_zero.py AtkinsonHyperlegibleNext-wght.ttf AtkinsonHyperlegibleMono-wght.ttf

Each file is rewritten in place. A font whose `zero` already has two contours is skipped,
so running the script again changes nothing. To start over, restore the upstream fonts first.

The fonts are licensed under the SIL Open Font License 1.1, with no Reserved Font Name, and
the edited files are Modified Versions under that same license (see README.md).

Needs fontTools (tested with 4.60).
"""

import sys
from array import array

from fontTools.misc.roundTools import otRound
from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._g_l_y_f import GlyphCoordinates

GLYPH = "zero"
OUTER = 0  # index of the outer contour
ON_CURVE = 0x01


def contour_ranges(end_points):
    """Return (first, last) point indices for each contour."""
    ranges = []
    first = 0
    for last in end_points:
        ranges.append((first, last))
        first = last + 1
    return ranges


def extremes(coords, indices):
    """Return the indices of the leftmost, rightmost, lowest and highest points."""
    return (
        min(indices, key=lambda i: coords[i][0]),
        max(indices, key=lambda i: coords[i][0]),
        min(indices, key=lambda i: coords[i][1]),
        max(indices, key=lambda i: coords[i][1]),
    )


def box(points, ext):
    """Return (centre x, centre y, width, height) from the four extreme points."""
    left, right, bottom, top = (points[i] for i in ext)
    return (
        (left[0] + right[0]) / 2,
        (bottom[1] + top[1]) / 2,
        right[0] - left[0],
        top[1] - bottom[1],
    )


def inner_from_outer(points, outer_order, outer_box, inner_box):
    """Scale the outer points (in `outer_order`) so their box becomes `inner_box`."""
    ocx, ocy, ow, oh = outer_box
    icx, icy, iw, ih = inner_box
    sx, sy = iw / ow, ih / oh
    return [
        (icx + sx * (points[i][0] - ocx), icy + sy * (points[i][1] - ocy)) for i in outer_order
    ]


def plain_zero(font):
    """Edit `zero` in `font`. Return a short report, or None if it was already plain."""
    glyf = font["glyf"]
    gvar = font["gvar"]
    hmtx_before = dict(font["hmtx"].metrics)
    glyph = glyf[GLYPH]
    if glyph.isComposite() or glyph.numberOfContours != 3:
        if glyph.numberOfContours == 2:
            return None
        raise ValueError(f"unexpected {GLYPH}: {glyph.numberOfContours} contours")

    orig, controls = glyf._getCoordinatesAndControls(GLYPH, font["hmtx"].metrics, None)
    orig = [tuple(p) for p in orig]
    ranges = contour_ranges(controls.endPts)
    o_first, o_last = ranges[OUTER]
    n_outer = o_last - o_first + 1
    outer_ids = list(range(o_first, o_last + 1))
    inner_ids = [i for first, last in ranges[1:] for i in range(first, last + 1)]
    phantom_ids = list(range(len(orig) - 4, len(orig)))
    outer_ext = extremes(orig, outer_ids)
    inner_ext = extremes(orig, inner_ids)

    # Reverse the direction but keep the first (on-curve) point first.
    order = [o_first] + list(range(o_last, o_first, -1))
    flags = [controls.flags[i] & ON_CURVE for i in order]
    if not flags[0]:
        raise ValueError("outer contour does not start on an on-curve point")

    base_inner = inner_from_outer(orig, order, box(orig, outer_ext), box(orig, inner_ext))
    base_inner = [(otRound(x), otRound(y)) for x, y in base_inner]
    scales = [scale_report("default", box(orig, outer_ext), box(orig, inner_ext))]

    for var in gvar.variations.get(GLYPH, []):
        var.calcInferredDeltas(orig, controls.endPts)
        deltas = var.coordinates
        moved = [(p[0] + d[0], p[1] + d[1]) for p, d in zip(orig, deltas)]
        outer_box, inner_box = box(moved, outer_ext), box(moved, inner_ext)
        new_inner = inner_from_outer(moved, order, outer_box, inner_box)
        inner_deltas = [
            (otRound(x) - bx, otRound(y) - by) for (x, y), (bx, by) in zip(new_inner, base_inner)
        ]
        var.coordinates = (
            [deltas[i] for i in outer_ids] + inner_deltas + [deltas[i] for i in phantom_ids]
        )
        scales.append(scale_report(f"wght peak {var.axes['wght'][1]:+.3f}", outer_box, inner_box))

    glyph.coordinates = GlyphCoordinates([orig[i] for i in outer_ids] + base_inner)
    glyph.flags = array("B", [controls.flags[i] & ON_CURVE for i in outer_ids] + flags)
    glyph.endPtsOfContours = [n_outer - 1, 2 * n_outer - 1]
    glyph.numberOfContours = 2
    glyph.recalcBounds(glyf)

    if dict(font["hmtx"].metrics) != hmtx_before:
        raise AssertionError("hmtx changed")
    if glyph.xMin != font["hmtx"][GLYPH][1]:
        raise AssertionError("left side bearing no longer matches xMin")
    return scales


def scale_report(label, outer_box, inner_box):
    """Return 'label: sx=..., sy=...' for the report."""
    return f"{label}: sx={inner_box[2] / outer_box[2]:.4f}, sy={inner_box[3] / outer_box[3]:.4f}"


def main(paths):
    for path in paths:
        font = TTFont(path)
        scales = plain_zero(font)
        if scales is None:
            print(f"{path}: {GLYPH} is already plain, skipped")
            continue
        font.save(path)
        print(f"{path}: {GLYPH} now has a plain counter")
        for line in scales:
            print(f"  {line}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    main(sys.argv[1:])
