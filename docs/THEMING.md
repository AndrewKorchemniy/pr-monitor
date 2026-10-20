# Theming

Every colour, radius, spacing and motion value comes from a theme. A theme is one TOML file.
The three built-in themes are in `crates/prm-theme/themes/`:

| Theme | Appearance | Character |
|---|---|---|
| Graphite | dark (default) | graphite pencil on a dark drafting table |
| Vellum | light | drafting film; the default light theme |
| Cyanotype | dark | a blueprint: square corners, no stripes |

## Making your own

1. In Settings › Appearance, choose **Copy theme as TOML** on the theme closest to what you
   want.
2. Save it as `%LOCALAPPDATA%\PRMonitor\themes\<id>.toml`. **Copy themes folder path** gives
   you the folder.
3. Set `id` to the file name (without `.toml`) and give it a `name`.
4. Change the seeds. PR Monitor reloads the folder whenever its window gains focus, and
   Settings › Appearance › **Reload themes** reloads it on demand.

```toml
schema = 1
id = "midnight"
name = "Midnight"
appearance = "dark"            # "dark" | "light"

[seeds]
base = "#101418"               # window and panels
ink = "#E4E8EE"                # primary text
accent = "#7FB8FF"             # focus, selection, links, the change bar
contrast = 0.0                 # -1.0..=1.0

[seeds.status]
needs_review = "#E7B551"
active_comments = "#EF8467"
reviewed = "#82C79D"

[metrics]
radius_scale = 1.0             # 0 = square corners
stripes = true

[overrides]
"row.selected" = "#1E2A36"     # any palette key
```

## How a theme becomes colours

- **Derived tokens.** About forty semantic tokens are derived from the seeds in OKLCH:
  surfaces, borders, text levels, row hover and selection, control states, and a fill and
  foreground for each status. They are derived, not hand-picked, so a theme stays coherent
  when you change only `base` or `accent`.
- **Overrides.** `[overrides]` replaces individual tokens after derivation.
- **Contrast checks.** Derived text and status marks are adjusted until they reach the
  targets: 4.5:1 for text, 3:1 for muted text. Every theme, overrides included, is then
  checked against the same rules, and any failure is listed in Settings › Appearance.
  Built-in themes must pass with no issues (a test enforces it).
- **Bad files.** A theme that fails to parse is listed in Settings › Appearance with its
  error and skipped. The app never refuses to start because of a theme.

## Following Windows

Choosing **Match Windows** uses one theme for light mode and one for dark mode (by default
Vellum and Graphite). The choice is stored as `ui.theme = "system"`, with `ui.theme_dark` and
`ui.theme_light`, in `config.toml`.

## For contributors

- `prm-ui/src/theme/adapter.rs` is the **only** place that turns tokens into egui colours or
  styles. A test fails if a colour literal appears anywhere else in the UI.
- Status is never shown by colour alone. Each status also has its own painted glyph: a
  ring, a diamond or a check.
- The UI font (Atkinson Hyperlegible Next and Mono) has no arrow glyphs. Sort arrows and
  other symbols are painted icons, and key hints use words ("Down", not "↓").
- Adding a built-in theme means adding one TOML file to `crates/prm-theme/themes/`.
  `build.rs` embeds every file in that folder.
