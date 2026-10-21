# Bundled fonts

| File | Upstream | License |
| --- | --- | --- |
| `AtkinsonHyperlegibleNext-wght.ttf` | [Atkinson Hyperlegible Next](https://github.com/googlefonts/atkinson-hyperlegible-next) | [OFL-AtkinsonHyperlegibleNext.txt](OFL-AtkinsonHyperlegibleNext.txt) |
| `AtkinsonHyperlegibleMono-wght.ttf` | [Atkinson Hyperlegible Mono](https://github.com/googlefonts/atkinson-hyperlegible-next-mono) | [OFL-AtkinsonHyperlegibleMono.txt](OFL-AtkinsonHyperlegibleMono.txt) |

Both files are **Modified Versions** of the upstream variable fonts, as defined by the SIL
Open Font License 1.1. They stay under that license; the OFL texts next to them apply in full.
Neither font declares a Reserved Font Name.

## What changed

Only the `zero` glyph. Upstream, it has a diagonal slash. Here it is a plain oval: the two
halves of the counter are replaced by one counter with the same size at every weight, so the
stroke weights stay as designed. Advance widths, kerning and every other glyph are unchanged.
In Next, `zero.tf` (tabular figures) is built from `zero`, so it changes too. In Mono,
`zero.zero` (the alternate for the `zero` OpenType feature) is left as it is.

The reason is taste: the slashed zero is the only form either font offers in its default
glyph, and egui cannot turn on OpenType features to pick another.

## Regenerating

`plain_zero.py` makes the change. It needs Python 3 and fontTools.

1. Put the upstream variable fonts here under the file names above.
2. From this directory, run:

   ```sh
   python plain_zero.py AtkinsonHyperlegibleNext-wght.ttf AtkinsonHyperlegibleMono-wght.ttf
   ```

The script rewrites each file in place and skips a font whose `zero` is already plain, so
running it twice is harmless.
