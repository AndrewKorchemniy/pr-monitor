# Binary assets

- `app.manifest` (required): DPI awareness, UTF-8 code page, segment heap, Common Controls
  v6 and supported OS versions. It is embedded by `build.rs` through winresource.

The icons are generated at build time from the theme's brand mark (`prm_theme::render_mark`,
Graphite colours). To use hand-made art instead, drop either of these files here:

- `app.ico`: the exe icon (multi-size: 16 through 256 px).
- `icon-256.png`: the toast icon, also written to the data folder at startup.

The window and tray icons are rendered at run time, so they follow the theme and the tray
state. They have no files to override.
