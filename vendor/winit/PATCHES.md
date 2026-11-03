# Local changes to winit

This is winit 0.30.13 from crates.io (upstream commit `e9809ef54b18499bb4f2cac945719ecc2a61061b`,
see `.cargo_vcs_info.json`), used through `[patch.crates-io]` in the workspace `Cargo.toml`.
`vendor/` is in the workspace `exclude` list, so it is not formatted, linted or tested with our
code, and `_typos.toml` skips it.

To see exactly what differs from the published crate:

```sh
cargo fetch --locked
diff -ru ~/.cargo/registry/src/index.crates.io-*/winit-0.30.13 vendor/winit
```

(The first commit that added this directory is the pristine copy, so
`git diff <that commit> HEAD -- vendor/winit` shows the same thing.)

## Patch: a stale monitor handle must not panic

Upstream issue: <https://github.com/rust-windowing/winit/issues/3102> ("Invalid monitor handle",
`Os { code: 1461 }`). An `HMONITOR` goes stale when the display topology changes (Win+P, a monitor
unplugged, session lock/unlock, remote-desktop reconnect). egui-winit asks
`current_monitor().size()` every frame, so the app crashed.

`src/platform_impl/windows/monitor.rs`, in the `MonitorHandle` impl:

| Function | Before | Now |
|---|---|---|
| `name()` | `get_monitor_info(..).unwrap()` | `.ok()?` (its docs already say `None` when the monitor is gone) |
| `native_identifier()` | `self.name().unwrap()` | `self.name().unwrap_or_default()` (public return type is `String`) |
| `size()` | `get_monitor_info(..).unwrap()` | `0x0` when the lookup fails, as `position()` already does for `(0, 0)` |

Doc lines added in `src/monitor.rs` (`size`) and `src/platform/windows.rs` (`native_id`).

## Local deviations that are not for upstream

- `Cargo.toml`: `[lints.rust] warnings = "allow"`. CI sets `CARGO_BUILD_WARNINGS=deny`, which
  applies to path crates outside the workspace; this keeps a newer toolchain's warnings in
  winit's Windows, Linux and macOS backends from failing our builds.
- `PATCHES.md` (this file).

## Known remaining unwrap, not patched

`src/platform_impl/windows/window.rs`, `set_fullscreen`: `monitor::get_monitor_info(..).unwrap()`
in the exclusive-fullscreen mode change, next to `assert_eq!(res, DISP_CHANGE_SUCCESSFUL)`
checks that also panic on OS failures. PR Monitor never uses exclusive fullscreen, so it is not
reachable here; left alone to keep this diff small and upstream-ready.

## Security advisories

Because winit is now a path source, `cargo deny check advisories` (audit.yml) may not match
RustSec advisories against it. Watch the winit advisories on <https://rustsec.org/packages/winit.html>
(0.30.x in particular) and the winit releases by hand, and treat a relevant one as a reason to
remove this directory.

## Drop condition

Delete `vendor/winit`, the `exclude = ["vendor"]` entry and the `[patch.crates-io]` entry when
winit fixes rust-windowing/winit#3102 in a release that egui-winit resolves (a 0.30.x backport,
or the 0.31 line once eframe moves to it). Do not re-vendor a newer copy: delete the patch. If
`cargo` reports `Patch winit was not used in the crate graph`, the patch is already obsolete.
