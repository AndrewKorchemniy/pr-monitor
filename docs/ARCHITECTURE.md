# Architecture

PR Monitor is one executable built from nine crates: pure logic at the bottom, I/O in the
middle, the window at the top. Every boundary is a trait ("port"), so logic is testable without
a network, credential store or screen.

## Crates

```
prm-domain   pure: ids, models, classification, diffing, notification planning, the view model, update status, the Updater port
prm-theme    pure: theme files, OKLab derivation, contrast checks, the brand mark
  prm-config   config.toml / state.json: schema, migration, validation, atomic writes (uses prm-theme's `Corners`)
  prm-ado      Azure DevOps REST client: transport, paging, retries, rate limits, DTO mapping
    prm-auth   token sources (Azure CLI, PAT, Entra), single-flight cache, secret storage
  prm-service  the actor: scheduler, refresh jobs, update checks, notifications, the published AppState
    prm-platform  OS adapters: toasts, credential store, single instance and second-launch handoff, autostart, DWM
    prm-ui        egui: dashboard, settings, tray, keymap, theme runtime (no platform code)
      pr-monitor  the binary: startup order, wiring, renderer choice, logging, panics, the Velopack updater
```

`prm-ui` does not depend on `prm-platform`. The binary bridges the two through the
`prm_ui::UiHost` trait (title bar colours, window corner shape, cloaking, the OS animation setting).

## Threads and data flow

```
UI thread (winit + eframe + tray)            "svc" thread (tokio, current_thread)
  CommandSender::send(Command)  ───────────▶  service actor (owns all mutable state)
  watch::Receiver<Arc<AppState>> ◀─────────  publishes a new snapshot after each change
  mpsc::Receiver<AppEvent>      ◀─────────  people search, sign-in, save and test results
  ctx.request_repaint()         ◀─────────  every event also wakes the UI
```

- The actor never awaits I/O in a command handler. It changes state and spawns jobs, one
  per organization, so a failing org never blocks the others.
- The UI never formats per frame. Rows (`PrRow`) are built once per data change, with
  `rows_generation` telling the UI when to rebuild its view.
- `logic()` runs even while the window is hidden. It drains events, the tray and
  activations. `ui()` only draws.
- Nothing repaints while hidden. Visible windows wake once a minute (for ages), every second
  in the last minute before a refresh when focused, and at deadlines (debounced saves, the end
  of a notification pause).
- There are no transient in-app messages. A result shows where the user is already looking
  (a banner, the settings Save bar, a line beside the button that asked, the people picker's
  dropdown, the repository picker); anything else only goes to the log.

## A refresh

1. **Plan.** Each org's scope becomes the fewest queries: one per project, or one per repo
   when only a few repos of a project are chosen.
2. **List.** Fetch the active PRs, keep tracked authors, and harvest the people seen. An
   empty people list tracks every author in scope; "include my own pull requests" decides
   whether the signed-in user's PRs are among them (`OrgProfile::track_spec`).
3. **Threads.** Fetch the threads of each tracked PR, at most `THREAD_READ_BUDGET` per
   refresh. Beyond that new PRs go first, then the user's PRs (as author or reviewer), then
   the least recently read (`plan_thread_reads`). The rest keep their earlier threads. New
   PRs that did not fit, or whose read failed, wait for a later refresh, so a PR never shows
   without its threads; while the budget holds some back, a banner says so and the next
   refresh comes after `CATCH_UP_DELAY` instead of the interval. A first load or rebaseline
   stays silent until the PRs the budget held back are in.
4. **Classify.** Active comments beat Reviewed, which beats Needs review (`classify.rs`).
5. **Diff** against the last snapshot: new PRs, votes, comments, pushes, closes.
6. **Notify.** Plan toasts (budget per refresh, grouping, focus rules) and show them on the
   notifier thread.
7. **Persist** `state.json` and publish a new `AppState`.

## Startup (`pr-monitor/src/startup.rs`)

1. Run Velopack's hooks (`update::boot`: the installer and updater start the app with
   `--veloapp-*` flags that they answer and exit on), then parse arguments. `--version`,
   `--help` and `--self-test` exit here.
2. Resolve the data folders (`--home` or `PRMONITOR_HOME` for one folder, else settings in
   `%APPDATA%\pr-monitor` and local files in `%LOCALAPPDATA%\pr-monitor`). On the default
   folders, `prm-config::legacy` moves an old `%LOCALAPPDATA%\PRMonitor` folder over first.
3. Take the single-instance lock. A second launch forwards (show, or open a link) and exits
   before any logging or GPU work.
4. Create folders, the clock, logging and the panic hook.
5. Register the shell identity: toast AUMID, the `prmonitor://` protocol, the icon file.
6. Apply a downloaded update if one is waiting: hand it to Velopack's Update.exe, which waits
   for this process to exit, swaps the files and starts the app again; then exit. This is
   done only by the primary (a second launch would stop the running one), once logging
   exists (a failure is recorded), and before the listener, the service or any window.
7. Start the activation listener. Activations are buffered until the service exists.
8. Load the config, state and catalogs. Build the platform adapters and the transport (the
   proxy is resolved on a thread).
9. Spawn the service, before the window, so the first frame paints the cached snapshot.
10. Choose the renderer (OpenGL by default; wgpu over remote desktops, on arm64, or after an
   OpenGL failure) and run eframe.
11. On exit: stop the service (it persists), stop the handoff listener, release the lock. If the renderer
    failed to start, relaunch once with the other one.

## Updates

- Packages are Velopack releases on the repository's GitHub releases (installer, or the portable
  zip, both updating in place). The `Updater` port (`prm-domain`) is blocking; the binary
  implements it over `UpdateManager` (`pr-monitor/src/update.rs`).
- The service owns the schedule: a check shortly after start, then every few hours, a retry
  after a failure, each call with its own timeout (Velopack's HTTP client has none). It
  downloads in the background and publishes `AppState.update` (`UpdateStatus`).
- A downloaded update is applied on the next start, or at once from About › Restart to update:
  `Command::ApplyUpdate` starts Update.exe, then `AppEvent::QuitForUpdate` makes the UI quit
  through the normal graceful path, so state is saved and the lock released before files swap.
- `[updates] auto = false` stops the background checks only; "Check now" and applying a package
  that is already downloaded still work. A copy that Velopack did not install (`cargo run`, a
  bare exe) or one started with `--home` reports `Disabled` and never calls the updater.
- Debug builds read `PRMONITOR_UPDATE_FEED=<dir>` to update from a local folder of packages.

## Testing

- Tests live next to the code in `<module>/tests.rs`.
- The domain is covered by pure tests. The service runs against fakes with paused tokio
  time. The ADO client runs against wiremock. The UI uses egui_kittest with accessibility
  checks and missing-glyph detection.
- Nothing in the test suite touches the network, the credential store, the registry or
  toasts. Those are `#[ignore]` smoke tests, run by hand.
- `pr-monitor --self-test` checks what the exe embeds (themes, icon, toast XML, codecs,
  redaction) without writing anything.

## Code style

See [RUST_STYLE.md](RUST_STYLE.md).
