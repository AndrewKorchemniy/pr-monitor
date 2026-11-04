# Releasing

`.github/workflows/release.yml` builds releases and publishes them as GitHub releases tagged
`v<version>` (the version comes from `crates/pr-monitor/Cargo.toml`). A release starts as a
draft, gets its assets and attestations while still a draft, and goes public only when a
maintainer approves the `go-live` job. Run a dry run before every release.

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

`go-live` runs in the `release` environment and only publishes the draft: as the latest
release, or as a pre-release when the version has a suffix such as `-beta.2`. GitHub creates the
tag on the target commit at that point.

## Manual release

1. In a PR, bump `version` in `crates/pr-monitor/Cargo.toml` and run `cargo build --workspace`
   so `Cargo.lock` follows (CI uses `--locked`). Merge it.
2. Run the workflow on `main` with `mode=dry-run`; run the exe from the artifact and read the
   job summary.
3. Run it on `main` with `mode=release-current`.
4. When `publish` is done, check the draft release, then approve `go-live`.

Release notes come from `.github/scripts/release-notes.sh` plus the footer from
`.github/scripts/release-footer.sh` (install and verify instructions). For each PR merged since
the last published release the notes show its title, its description's "What and why" and its
squashed commit titles. Preview with
`PREV_TAG=v0.2.0 bash .github/scripts/release-notes.sh AndrewKorchemniy/pr-monitor <sha> v0.2.1`.
`CHANGELOG.md` is updated only on the release-plz path.

## release-plz (optional)

It needs a GitHub App, because PRs opened with `GITHUB_TOKEN` do not trigger CI (and the
required `ci-ok` check).

1. Create an App with Contents and Pull requests (read and write) and install it on the repo.
2. Add the variable `RELEASE_APP_CLIENT_ID` and the secret `RELEASE_APP_PRIVATE_KEY`.

On every push to `main`, `release-plz PR` opens or updates a `chore: release v<version>` PR
(label `release`, version bump, `CHANGELOG.md`); only `feat`, `fix`, `perf` and breaking
commits open one (`release_commits` in `release-plz.toml`). Merging it makes `release-plz
release` create the tag and a draft release, and the build, publish and `go-live` jobs follow as
in `release-current`. Dry-run on `main` before merging. release-plz reads versions from git tags
(`git_only = true`), so it continues from the last `v*` tag; never run `release-current` for a
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

There is no bare exe (the app cannot update itself outside an install) and no macOS build
(Gatekeeper blocks unsigned apps).

**Install and data folders.** The package id is `pr-monitor`. Settings (`config.toml`,
`keymap.toml`, `themes`) live in `%APPDATA%\pr-monitor`, so Velopack never touches them; the
local files (state, cache, logs) share the install folder `%LOCALAPPDATA%\pr-monitor`. Uninstall
removes the program and the local files but keeps the settings; `--unregister --purge` removes
both folders but leaves Velopack's own entries. The app moves a legacy `%LOCALAPPDATA%\PRMonitor`
folder into these two on first start. A Portable and an installed copy share the data folders
and the single-instance key, so only one runs at a time.

## How the packages are built

Jobs: `release` → `notices` → `build` ×4 → `package` ×4 → `publish` → `go-live` → `winget`.
`build` and `package` are matrices over Windows and Linux.

- **`build`** compiles each binary on its own architecture. Windows runs `verify-exe.ps1` (PE
  machine, GUI subsystem, 24 MiB size budget, static CRT, `ProductVersion`, headless
  `--self-test`) and stages the icon `build.rs` generated. Linux uses `ubuntu-22.04` and
  `ubuntu-22.04-arm`; `verify-elf.sh` checks the ELF machine, size (a provisional 40 MiB), the
  highest glibc symbol (at most 2.35), the version and `--self-test`. No `RUSTFLAGS`
  anywhere: `.cargo/config.toml` sets `+crt-static` for the Windows targets only.
- **`package`** runs `.github/scripts/pack-velopack.ps1`: both Windows runtimes on
  `windows-latest` (vpk cross-packs), each Linux runtime on a runner of its own architecture so
  `smoke-appimage.sh` can run the AppImage (`--appimage-extract-and-run --self-test`; runners
  have no FUSE). It installs the pinned `vpk`, stages the exe and licences, downloads the
  previous **published** release for the delta (anonymously, so this release's own draft is not
  listed; none is fine), runs `vpk pack` (on Linux `vpk [linux] pack`; `--aumid` and
  `--shortcuts` are Windows-only), cuts `releases.<rid>.json` down to this version and runs
  `verify-package.ps1`. The pack script's `$Aumid` must equal `prm_domain::app::AUMID`.
- **`publish`** attests, checksums and uploads to the **draft** with `gh release upload` (not
  `vpk upload github`, which refuses a draft that already holds the feed and would break
  `recover`). Only `go-live` publishes.

**How updates reach users.** The app reads the feeds of the 10 newest published,
non-pre-release releases. Drafts and pre-releases are never offered, so `go-live` alone decides
what users get.

**Bumping vpk.** `$VpkVersion` in `pack-velopack.ps1` must equal the `velopack` crate in
`Cargo.lock`; the script fails otherwise (only a `-NoDownload` dry run tolerates a missing
crate). Dependabot bumps the crate; bump `$VpkVersion` in the same PR. The CI jobs `build
(windows-x64, release)`, `build (windows-arm64, release)` and `build (linux-x64, release)` run
the script as a dry run, so mismatches show on the PR.

**Packing locally** (needs the .NET SDK and a release build):

```powershell
cargo build --release -p pr-monitor
./.github/scripts/pack-velopack.ps1 -Rid win-x64 -Exe target/release/pr-monitor.exe -OutDir pack-out -NoDownload
```

On Linux pass the Linux rid, the binary and `-Version` (a Linux binary has no ProductVersion):
`pwsh ./.github/scripts/pack-velopack.ps1 -Rid linux-x64 -Version 0.3.0 -Exe target/release/pr-monitor -OutDir pack-out -NoDownload`.

**Linux facts.** Updates stage in `/var/tmp`; an AppImage in a privileged folder prompts via
`pkexec`. The first update after an install is a full download. There is no uninstall hook:
delete the AppImage (`pr-monitor --unregister` removes the autostart entry first). Autostart
uses `$APPIMAGE` (when `$APPDIR` confirms the binary is inside it), because `current_exe()`
inside an AppImage points into a temporary mount. Velopack's log is
`/tmp/velopack_pr-monitor.log`.

## winget

The first submission of `AndrewKorchemniy.PRMonitor` to winget-pkgs is manual (the `winget` job
only updates an existing package): run `wingetcreate new` with the `Setup.exe` URLs and set
`InstallerType: exe`, `Scope: user` and `InstallerSwitches` `Silent` and `SilentWithProgress`
to `--silent`. Install it once from a local manifest before enabling `WINGET_ENABLED`. The job
then picks `pr-monitor-win-<arch>-Setup.exe` from each release.

## When something fails

- **Before `go-live`:** the release is still a draft. `mode=recover` with the tag rebuilds the
  commit it was created from and re-uploads the assets with `--clobber`. Run it from `main` so a
  workflow fix or a retry takes effect; a code fix is not picked up (bump the version and
  release again).
- **After `go-live`:** the tag can never be deleted or moved (`tags.json`). Bump the patch
  version and release again.
- **`release-current` says the release exists:** bump the version; a version is released once.
- **An install or update misbehaves on a user's machine:** `Update.exe` logs to
  `%LocalAppData%\velopack\velopack_pr-monitor.log`; the app's logs are in
  `%LOCALAPPDATA%\pr-monitor\logs`.

## Verifying a release

```powershell
gh release download v0.1.0 -R AndrewKorchemniy/pr-monitor
gh attestation verify pr-monitor-win-x64-Setup.exe -R AndrewKorchemniy/pr-monitor
gh attestation verify pr-monitor-win-x64-Setup.exe -R AndrewKorchemniy/pr-monitor --predicate-type https://cyclonedx.org/bom
Get-FileHash -Algorithm SHA256 pr-monitor-win-*   # compare with SHA256SUMS.txt
```

## Settings

Nothing is needed for a plain release. These switch optional jobs on; the `go-live` and
`winget` jobs run in the `release` environment, so put the winget secret and variable there.

| Name | Kind | Effect |
|---|---|---|
| `WINGET_ENABLED` | variable | `true` enables the `winget` job, which submits the release as `AndrewKorchemniy.PRMonitor`. Skipped in dry runs. |
| `WINGET_TOKEN` | secret | Classic PAT with `public_repo` and `workflow` scopes, for the winget job. |
| `RELEASE_APP_CLIENT_ID` | variable | Enables the release-plz PR job. |
| `RELEASE_APP_PRIVATE_KEY` | secret | The App's private key. |
