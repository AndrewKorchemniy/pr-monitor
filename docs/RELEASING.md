# Releasing

`.github/workflows/release.yml` builds releases and publishes them as GitHub releases tagged
`v<version>` (the version comes from `crates/pr-monitor/Cargo.toml`). A release starts as a draft,
gets its assets and attestations while still a draft, and goes public only when a maintainer
approves the `go-live` job. Run a dry run before every release.

Two ways to start one:

- **Manual:** bump the version in a PR, then run the workflow in `release-current` mode.
- **release-plz (optional):** with a GitHub App configured, release-plz opens a release PR on
  every push to `main`; merging it starts the release.

## One-time setup

1. Check that nothing secret is tracked:
   `git ls-files | grep -Ei '\.(log|pfx|pem|env)$|secret|token'` and `git log --all --stat`.
2. Create the repository and push. It must be public (attestations, free `windows-11-arm`
   runners and Scorecard publishing need it).
3. Import the rulesets:
   `gh api -X POST repos/AndrewKorchemniy/pr-monitor/rulesets --input .github/rulesets/main.json`,
   then the same with `tags.json` (it blocks deleting or moving `v*` tags).
4. Turn on private vulnerability reporting (SECURITY.md links to it):
   `gh api -X PUT repos/AndrewKorchemniy/pr-monitor/private-vulnerability-reporting`.
5. In **Settings › Environments**, create `release`: add AndrewKorchemniy as required reviewer
   and limit deployment branches to `main`. This gates `go-live`; the winget secrets live here too.
6. Optional: turn on **Immutable releases** (assets are uploaded while the release is a draft,
   so this works).
7. Run `dry-run`, check the `release-dry-run` artifact, run `release-current`, approve `go-live`.

## Workflow modes

Run **Actions › Release › Run workflow**, or `gh workflow run release.yml -f mode=<mode>`.

| Mode | What it does |
|---|---|
| `dry-run` (default) | Builds, verifies and packs the Windows and Linux binaries from the selected commit and uploads them with `SHA256SUMS.txt` as the `release-dry-run` artifact (kept 7 days). No release, tag or attestations. |
| `release-current` | Must run on `main`. Fails if release `v<version>` already exists. Otherwise creates a draft targeting the current commit, builds, packs, uploads assets and attestations to the draft, then waits for `go-live`. |
| `recover` | Takes `tag` (for example `v0.3.0`). Rebuilds and re-uploads the assets of that **draft** release, then waits for `go-live`. |

`go-live` runs in the `release` environment and publishes the draft: as the latest release, or as
a pre-release when the version has a suffix such as `-beta.2`. For a `release-current` draft,
publishing creates the tag on the target commit; release-plz pushes its tag together with the draft.

## Manual release

1. In a PR, bump `version` in `crates/pr-monitor/Cargo.toml` and run `cargo build --workspace`
   so `Cargo.lock` follows (CI uses `--locked`). Merge it.
2. Run the workflow on `main` with `mode=dry-run`; run the exe from the artifact and read the
   job summary.
3. Run it on `main` with `mode=release-current`.
4. When `publish` is done, check the draft release, then approve `go-live`.

Release notes come from `.github/scripts/release-notes.sh` (one heading per PR merged since the
last published release, with its "What and why" and its squashed commit titles) and
`.github/scripts/release-footer.sh` (install and verify text). Preview with
`PREV_TAG=v0.2.0 bash .github/scripts/release-notes.sh AndrewKorchemniy/pr-monitor <sha> v0.2.1`.
`CHANGELOG.md` is updated only on the release-plz path.

## release-plz (optional)

It needs a GitHub App, because PRs opened with `GITHUB_TOKEN` do not trigger CI (or the required
`ci-ok` check).

1. Create an App with Contents and Pull requests (read and write) and install it on the repo.
2. Add the variable `RELEASE_APP_CLIENT_ID` and the secret `RELEASE_APP_PRIVATE_KEY`.

On every push to `main`, `release-plz PR` opens or updates a `chore: release v<version>` PR
(label `release`, version bump, `CHANGELOG.md`). Only `feat`, `fix`, `perf` and breaking commits
open one (`release_commits` in `release-plz.toml`). Merging it makes `release-plz release` create
the tag and a draft release; the build, publish and `go-live` jobs then follow as in
`release-current`. Dry-run on `main` before merging. release-plz reads versions from git tags
(`git_only = true`), so it continues from the last `v*` tag. Never run `release-current` for a
version a release-plz PR is about to release.

## What a release contains

Built with [Velopack](https://velopack.io) (`vpk`), package id `pr-monitor`, per architecture
(`x64`, `arm64`). `<rid>` is `win-<arch>` or `linux-<arch>`.

| Asset | What it is |
|---|---|
| `pr-monitor-win-<arch>-Setup.exe` | Per-user installer into `%LOCALAPPDATA%\pr-monitor`, no admin rights. `--silent` installs silently. |
| `pr-monitor-win-<arch>-Portable.zip` | `PR Monitor.exe` (launcher), `Update.exe`, `current\pr-monitor.exe` and the licence files. |
| `pr-monitor-linux-<arch>.AppImage` | Linux (preview): self-updating AppImage; no installer, uninstall hook or zip. |
| `pr-monitor-<version>-<rid>-full.nupkg` | The update package (on Linux it holds the AppImage). |
| `pr-monitor-<version>-<rid>-delta.nupkg` | Patch from the previous published release (absent for the first release on a channel). |
| `releases.<rid>.json` | The update feed: this version's nupkgs only. |
| `pr-monitor-<version>-windows-<arch>.pdb` | Crash symbols (Windows only). |
| `pr-monitor-<os>-<arch>.cdx.json` | CycloneDX SBOM (`<os>` is `windows` or `linux`), attested to the Setup and Portable zip or the AppImage. |
| `THIRD-PARTY-LICENSES.html`, `SHA256SUMS.txt` | Licences; checksums of every other asset. |

There is no bare exe (the app cannot update itself outside an install) and no macOS build.

**Install and data folders.** Settings (`config.toml`, `keymap.toml`, `themes`) live in
`%APPDATA%\pr-monitor`, which Velopack never touches. The local files (state, cache, logs) live in
`%LOCALAPPDATA%\pr-monitor`, which is also the install folder. Uninstalling removes the program and
keeps the settings; `--unregister --purge` removes both folders but leaves Velopack's own entries.
On first start the app moves an old `%LOCALAPPDATA%\PRMonitor` folder into these two. A Portable
and an installed copy share both folders and the single-instance key, so only one runs at a time.

## How the packages are built

Jobs: `release` → `notices` → `build` ×4 → `package` ×4 → `publish` → `go-live` → `winget`.
`build` and `package` are matrices over Windows and Linux, x64 and arm64. `release-pr` runs the
release-plz PR job on pushes to `main`.

- **`build`** compiles each binary on its own architecture (`windows-latest` and
  `windows-11-arm`; `ubuntu-22.04` and `ubuntu-22.04-arm`). Windows runs `verify-exe.ps1` (PE
  machine, GUI subsystem, 24 MiB size budget, static CRT, `ProductVersion`, headless `--self-test`)
  and stages the icon `build.rs` generated. Linux runs `verify-elf.sh` (ELF machine, a provisional
  40 MiB size budget, highest glibc symbol at most 2.35, version, `--self-test`). Never set the
  `RUSTFLAGS` environment variable: it replaces the `+crt-static` flags in `.cargo/config.toml`,
  which apply to the Windows targets only.
- **`package`** runs `.github/scripts/pack-velopack.ps1` for each runtime. Both Windows runtimes are
  packed on `windows-latest` (vpk cross-packs). Each Linux runtime is packed on its own architecture,
  so `smoke-appimage.sh` can run the AppImage (`--appimage-extract-and-run --self-test`; runners
  have no FUSE). The script installs the pinned `vpk`, stages the exe and licences, downloads the
  previous **published** release for the delta (anonymously, so this release's own draft is not
  listed; none is fine), runs `vpk pack` (on Linux `vpk [linux] pack`; `--aumid` and `--shortcuts`
  are Windows-only), keeps only this version in `releases.<rid>.json`, and runs
  `verify-package.ps1`. Its `$Aumid` must equal `prm_domain::app::AUMID`.
- **`publish`** attests (provenance, plus an SBOM for each Setup, Portable zip and AppImage),
  checksums and uploads to the **draft** with `gh release upload`. It does not use
  `vpk upload github`, which refuses a draft that already holds the feed and would break `recover`.
  Only `go-live` publishes.

**How updates reach users.** The app reads the 10 newest GitHub releases, skipping pre-releases;
drafts are not listed to it. So `go-live` alone decides what users get.

**Bumping vpk.** `$VpkVersion` in `pack-velopack.ps1` must equal the `velopack` crate in
`Cargo.lock`; the script fails otherwise (a `-NoDownload` dry run tolerates a missing crate).
Dependabot bumps the crate, so bump `$VpkVersion` in the same PR. CI runs the script as a dry run
in the `build (windows-x64, release)`, `build (windows-arm64, release)` and
`build (linux-x64, release)` jobs, so a mismatch shows on the PR.

**Packing locally** (needs the .NET SDK and a release build):

```powershell
cargo build --release -p pr-monitor
./.github/scripts/pack-velopack.ps1 -Rid win-x64 -Exe target/release/pr-monitor.exe -OutDir pack-out -NoDownload
```

On Linux pass the Linux rid, the binary and `-Version` (a Linux binary has no ProductVersion):
`pwsh ./.github/scripts/pack-velopack.ps1 -Rid linux-x64 -Version 0.3.0 -Exe target/release/pr-monitor -OutDir pack-out -NoDownload`.

**Linux facts.** Updates stage in `/var/tmp`. An AppImage in a folder that needs root updates
through `pkexec`. The first update after an install is a full download. There is no uninstall
hook: delete the AppImage (`pr-monitor --unregister` removes the autostart entry first). Autostart
uses `$APPIMAGE` when `$APPDIR` confirms the binary is inside it, because `current_exe()` inside an
AppImage points into a temporary mount. Velopack's log is `velopack_pr-monitor.log` in the temp
folder (`/tmp` usually).

## winget

The first submission of `AndrewKorchemniy.PRMonitor` to winget-pkgs is manual (the `winget` job
only updates an existing package): run `wingetcreate new` with the `Setup.exe` URLs and set
`InstallerType: exe`, `Scope: user` and `InstallerSwitches` `Silent` and `SilentWithProgress` to
`--silent`. Install it once from a local manifest before enabling `WINGET_ENABLED`. The job then
picks `pr-monitor-win-<arch>-Setup.exe` from each release.

## When something fails

- **Before `go-live`:** the release is still a draft. `mode=recover` with the tag rebuilds the
  commit it was created from and re-uploads the assets with `--clobber`. Run it from `main` so a
  workflow fix takes effect; a code fix is not picked up (bump the version and release again).
- **After `go-live`:** the tag can never be deleted or moved (`tags.json`). Bump the patch version
  and release again.
- **`release-current` says the release exists:** bump the version; a version is released once.
- **An install or update misbehaves on a user's machine:** `Update.exe` logs to
  `%LocalAppData%\velopack\velopack_pr-monitor.log`; the app's logs are in
  `%LOCALAPPDATA%\pr-monitor\logs`.

## Verifying a release

Verify a download as described in [Download in the README](../README.md#download).

## Settings

Nothing is needed for a plain release. These switch optional jobs on; the `go-live` and `winget`
jobs run in the `release` environment, so put the winget secret and variable there.

| Name | Kind | Effect |
|---|---|---|
| `WINGET_ENABLED` | variable | `true` enables the `winget` job, which submits the release as `AndrewKorchemniy.PRMonitor`. Skipped in dry runs. |
| `WINGET_TOKEN` | secret | Classic PAT with `public_repo` and `workflow` scopes, for the winget job. |
| `RELEASE_APP_CLIENT_ID` | variable | Enables the release-plz PR job. |
| `RELEASE_APP_PRIVATE_KEY` | secret | The App's private key. |
