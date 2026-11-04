# Architecture

PR Monitor is one executable built from nine crates: pure logic at the bottom, I/O in the
middle, the window at the top. I/O sits behind traits (ports), so logic is testable without a
network, a credential store or a screen.

## Crates

```
prm-domain    pure: ids, models, classification, diffing, notification planning, view model,
              update status, the Updater port
prm-theme     pure: theme files, OKLab derivation, contrast checks, the brand mark
prm-config    config.toml and state.json: schema, migration, validation, atomic writes,
              the legacy folder move
prm-ado       Azure DevOps REST client: transport, paging, retries, rate limits, DTO mapping
prm-auth      token sources (Azure CLI, PAT, Entra), single-flight cache, secret storage
prm-service   the actor: scheduler, refresh jobs, update checks, notifications, AppState
prm-platform  OS adapters: toasts, credential store, single instance and second-launch
              handoff, autostart, DWM
prm-ui        egui: dashboard, settings, tray, keymap, theme runtime (no platform code)
pr-monitor    the binary: startup, wiring, renderer choice, logging, panics, Velopack updater
```

Normal dependencies:

- `prm-domain`, `prm-theme`: none
- `prm-config`: `prm-domain`, `prm-theme` (for `Corners`)
- `prm-ado`: `prm-domain`
- `prm-auth`: `prm-domain`, `prm-ado`
- `prm-service`: `prm-domain`, `prm-config`, `prm-ado`, `prm-auth`
- `prm-platform`: `prm-domain`, `prm-config`, `prm-auth`, `prm-service`
- `prm-ui`: `prm-domain`, `prm-config`, `prm-service`, `prm-theme`. Not `prm-platform`.
- `pr-monitor`: all of them.

The binary bridges `prm-ui` and `prm-platform` through the `prm_ui::UiHost` trait (title bar
colours, window corner shape, cloaking, the OS animation setting).

## Threads and data flow

```
UI thread (winit + eframe + tray)            "svc" thread (tokio, current_thread)
  CommandSender::send(Command)  ───────────▶  service actor (owns all mutable state)
  watch::Receiver<Arc<AppState>> ◀─────────  publishes a new snapshot after each change
  mpsc::Receiver<AppEvent>      ◀─────────  people search, sign-in, save and test results
  ctx.request_repaint()         ◀─────────  every event also wakes the UI
```

- The actor never awaits I/O in a command handler. It changes state and spawns jobs, one per
  organization, so a failing org never blocks the others.
- The UI never formats per frame. Rows (`PrRow`) are built once per data change, and
  `rows_generation` tells the UI when to rebuild its view.
- `logic()` runs even while the window is hidden: it drains events, the tray and activations.
  `ui()` only draws.
- Nothing repaints while hidden. A visible window repaints on each minute boundary, every second
  in the last minute before a refresh when focused (not in low-power mode), and at deadlines
  (debounced saves, the end of a notification pause).
- There are no transient in-app messages. A result is shown where the user already looks (a
  banner, the Settings save bar, a line beside the button that asked, a picker's dropdown);
  anything else only goes to the log.

## A refresh

1. **Plan.** `plan_queries` turns each org's scope into the fewest PR-list queries: one for the
   whole org in org-wide mode; otherwise one per project when all its repositories or two or
   more are chosen, and one per repository when exactly one is.
2. **List.** Fetch the active PRs, keep tracked authors, and harvest the people seen. An empty
   people list tracks every author in scope; "include my own pull requests" decides whether the
   signed-in user's PRs are among them (`OrgProfile::track_spec`).
3. **Threads.** Fetch the threads of each tracked PR, at most `THREAD_READ_BUDGET` (150) per
   refresh. Over budget, new PRs go first, then the user's PRs (as author or reviewer), then the
   least recently read (`plan_thread_reads`); the rest keep their earlier threads. A PR never
   shows without its threads: new PRs that did not fit, or whose read failed, wait for a later
   refresh. While the budget holds some back, a banner says so and the next refresh comes after
   `CATCH_UP_DELAY` (30 s) instead of the interval. A first load or rebaseline stays silent until
   the PRs the budget held back are in.
4. **Classify.** Active comments beat Reviewed, which beats Needs review (`classify.rs`).
5. **Diff** against the last snapshot: new PRs, votes, comments, pushes, closes.
6. **Notify.** Plan toasts (budget per refresh, grouping, focus rules) and show them on the
   toast thread.
7. **Persist** `state.json` and publish a new `AppState`.

## Startup (`crates/pr-monitor/src/startup.rs`)

1. Velopack's hooks run first (`update::boot`; the installer and updater start the app with
   `--veloapp-*` flags that it answers and exits on). Then the arguments are parsed.
   `--version`, `--help`, `--self-test` and `--unregister` exit here. The panic hook is installed
   before any of this.
2. The data folders are resolved: `--home` or `PRMONITOR_HOME` for one folder; otherwise settings
   in `%APPDATA%\pr-monitor` and local files in `%LOCALAPPDATA%\pr-monitor`.
3. The single-instance lock is taken. A second launch forwards (show, or open a link) and exits
   before any logging or GPU work. If the primary is quitting, the launch takes over once its lock
   is free.
4. The primary creates the folders and starts logging. On the default folders it first moves an
   old `%LOCALAPPDATA%\PRMonitor` folder over (`prm-config::legacy`), and it prunes old crash logs.
5. The shell identity is registered: toast AUMID, the `prmonitor://` protocol, the icon file.
6. A downloaded update is applied if one is waiting: Update.exe is started, waits for this process
   to exit, swaps the files and starts the app again; then this process exits. Only the primary
   does this, after the lock and the log, and before the listener, the service or any window.
7. The activation listener starts. Activations that arrive before the service exists are buffered.
8. The config, state and catalogs are loaded, and the platform adapters are built. The HTTP
   transport is created with the proxy resolved on a thread (`net::deferred_transport`).
9. The service is spawned before the window, so the first frame paints the cached snapshot.
10. The renderer is chosen: `--renderer`, then the setting, then wgpu on arm64 or over a remote
    session, wgpu after an OpenGL failure on this version, otherwise OpenGL. Then eframe runs.
11. On exit the service stops (it persists), the listener stops and the lock is released. A
    renderer that fails before its first frame is relaunched once with the other one.

## Updates

- Packages are Velopack releases on this repository's GitHub releases (installer, or the portable
  zip; both update in place). The `Updater` port (`prm-domain`) is blocking; the binary implements
  it over `UpdateManager` (`crates/pr-monitor/src/update.rs`).
- The service owns the schedule: the first check one minute after start, then every 6 hours, and
  a retry after an hour when a check fails. Each call has its own timeout (check 2 min, download
  30 min, apply 30 s), because Velopack's HTTP client has none. Downloads run in the background,
  and the result is published as `AppState.update` (`UpdateStatus`).
- A downloaded update is applied on the next start, or at once from About › Restart to update:
  `Command::ApplyUpdate` starts Update.exe, then `AppEvent::QuitForUpdate` makes the UI quit
  through the normal path, so state is saved and the lock is released before files are swapped.
- `[updates] auto = false` stops the automatic checks only; Check now and applying a downloaded
  package still work. A copy that Velopack did not install (`cargo run`, a bare exe), or one
  started with `--home`, reports `Disabled` and never calls the updater.
- Debug builds read `PRMONITOR_UPDATE_FEED=<dir>` to update from a local folder of packages.

## Testing

- Tests live next to the code in `<module>/tests.rs`.
- The domain has pure tests. The service runs against fakes with paused tokio time. The ADO
  client runs against wiremock. The UI uses egui_kittest, with accessibility checks on and
  missing glyphs failing the test.
- Nothing in the suite touches the network, the credential store, the registry or toasts. Those
  checks are `#[ignore]` smoke tests, run by hand on a desktop.
- `pr-monitor --self-test` checks what the exe embeds (config defaults, built-in themes, icon,
  toast XML, the handoff codec, URI round-trip, relaunch arguments, log redaction) and writes
  nothing.

## Code style

See [RUST_STYLE.md](RUST_STYLE.md).
