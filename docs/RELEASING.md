# Releasing

Releases are built by `.github/workflows/release.yml` and published as GitHub releases with
the tag `v<version>`. The version comes from `crates/pr-monitor/Cargo.toml`. Every release
starts as a draft, gets its assets and attestations while it is still a draft, and goes public
only after a maintainer approves the `go-live` job.

There are two ways to start a release:

- **Manual (no App needed):** bump the version in a PR, then run the workflow by hand in
  `release-current` mode. This is the path for v0.1.0 and works at any time.
- **release-plz (optional):** with a GitHub App configured, release-plz opens a release PR on
  every push to `main`, and merging it starts the release.

Run a **dry run before every release**, whichever path you use.

## One-time setup

Do these in order. Steps 1 to 6 happen once per repository.

1. Before the first push, check that nothing secret is tracked:
   `git ls-files | grep -Ei '\.(log|pfx|pem|env)$|secret|token'` and `git log --all --stat`.
2. Create the public repository and push. It must be public: attestations, the free
   `windows-11-arm` runners and Scorecard result publishing need it.
3. Import the rulesets from `.github/rulesets/`:
   `gh api -X POST repos/AndrewKorchemniy/pr-monitor/rulesets --input .github/rulesets/main.json`,
   then the same with `tags.json`. `tags.json` blocks deleting or moving any `v*` tag.
4. Turn on private vulnerability reporting (SECURITY.md links to it):
   `gh api -X PUT repos/AndrewKorchemniy/pr-monitor/private-vulnerability-reporting`.
5. In **Settings › Environments**, create `release`. Add AndrewKorchemniy as a required
   reviewer and limit deployment branches to `main`. This is the approval gate on `go-live`;
   the winget secrets also live in this environment.
6. Optional: turn on **Immutable releases**. Published releases and their assets then can no
   longer change, which is why assets are uploaded while the release is still a draft.
7. Run the workflow in `dry-run` mode and check the `release-dry-run` artifact.
8. Run it in `release-current` mode to create v0.1.0.
9. Approve the `go-live` job.

## Workflow modes

Run **Actions › Release › Run workflow**, or `gh workflow run release.yml -f mode=<mode>`.

| Mode | What it does |
|---|---|
| `dry-run` (default) | Builds and verifies the Windows and Linux binaries from the selected commit, packs them with Velopack, makes `SHA256SUMS.txt`, and uploads everything as the `release-dry-run` workflow artifact (kept 7 days). Creates no release, no tag and no attestations. |
| `release-current` | Must run on `main`. Reads the version from `crates/pr-monitor/Cargo.toml` and fails if a release `v<version>` already exists. Otherwise creates a draft release `v<version>` targeting the current commit, builds, packs, uploads assets and attestations to the draft, then waits for `go-live` approval. |
| `recover` | Takes `tag` (for example `v0.3.0`). Rebuilds and re-uploads the assets of an existing **draft** release for that tag, then waits for `go-live` approval. |

A draft release does not create the git tag. GitHub creates `v<version>` on the target commit
when `go-live` publishes the release.

`go-live` runs in the `release` environment and only marks the draft as published: as the
latest release, or as a pre-release when the version has a suffix such as `-alpha.1` or
`-beta.2`. Nothing becomes public until a reviewer approves it.

## What a release contains

Each release is built with [Velopack](https://velopack.io) (`vpk`). Per architecture (`x64`,
`arm64`; the Velopack channel and runtime are `win-x64`, `win-arm64`, `linux-x64` and
`linux-arm64`), with the package id `AndrewKorchemniy.PRMonitor`:

| Asset | What it is |
|---|---|
| `AndrewKorchemniy.PRMonitor-win-<arch>-Setup.exe` | One-click per-user installer (`%LOCALAPPDATA%\AndrewKorchemniy.PRMonitor`, no admin rights). Silent install: `--silent`. |
| `AndrewKorchemniy.PRMonitor-win-<arch>-Portable.zip` | `PR Monitor.exe` (launcher), `Update.exe` and `current\pr-monitor.exe` with the licence files. Unzip anywhere. |
| `AndrewKorchemniy.PRMonitor-linux-<arch>.AppImage` | Linux (preview): a self-updating AppImage. There is no installer, no uninstall hook and no zip. |
| `AndrewKorchemniy.PRMonitor-<version>-<rid>-full.nupkg` | The update package the app downloads when it is not on the previous version (`<rid>` is `win-<arch>` or `linux-<arch>`). For Linux it holds the AppImage itself. |
| `AndrewKorchemniy.PRMonitor-<version>-<rid>-delta.nupkg` | Patch from the previous published release (absent for the first release on that channel). |
| `releases.<rid>.json` | The update feed for that channel: lists this version's nupkgs only. |
| `pr-monitor-<version>-windows-<arch>.pdb` | Symbols for crash reports (Windows only). |
| `pr-monitor-<os>-<arch>.cdx.json` | CycloneDX SBOM, `<os>` is `windows` or `linux` (attested to the Setup and Portable zip, or to the AppImage). |
| `THIRD-PARTY-LICENSES.html`, `SHA256SUMS.txt` | Licences and checksums of every other asset. |

There is no bare exe any more: the app cannot update itself from outside an install. Linux is a
preview: it is built and smoke-tested like the Windows builds but used less. macOS is not built
(an unsigned, un-notarised app is blocked by Gatekeeper).

## How the packages are built

`.github/workflows/release.yml` runs `release` → `notices` → `build` ×4 → `package` ×4 →
`publish` → `go-live` → `winget`. `build` and `package` are matrices over Windows and Linux, so
the Linux builds share the draft, attestation and go-live gate with the Windows ones.

- `build` compiles each exe on its own architecture and runs `verify-exe.ps1` on it (PE
  machine, GUI subsystem, size budget, static CRT, `ProductVersion`, headless `--self-test`).
  It also stages the icon that `build.rs` generated, because the next job has no cargo.
- `build` for Linux uses `ubuntu-22.04` (x64) and `ubuntu-22.04-arm` (arm64), so the glibc floor
  is 2.35 on both. `.github/scripts/verify-elf.sh` checks the ELF machine, size (a provisional
  40 MiB budget until the first build is measured), the highest glibc symbol version (at most 2.35),
  the version and `--self-test`. There are no PDBs; the icon is the generated PNG. No `RUSTFLAGS` anywhere:
  `.cargo/config.toml` forces `+crt-static` only for the Windows targets.
- `package` runs `.github/scripts/pack-velopack.ps1` on `windows-latest` for both Windows
  runtimes (vpk cross-packs; the exes were already tested natively). The Linux runtimes pack on a
  runner of their own architecture (PowerShell 7 and .NET come with the Ubuntu images), so that
  `smoke-appimage.sh` can run the AppImage with `--appimage-extract-and-run --self-test` (the
  runners have no FUSE). On Linux the script uses `vpk [linux] pack` with a PNG icon and
  `--categories`; `--aumid` and `--shortcuts` exist only for Windows. The script installs the pinned `vpk`
  with `dotnet tool install`, stages the exe and licence files, downloads the previous
  **published** release for the delta (anonymous on purpose: a token would also list this
  release's own draft; having no previous release is fine), runs `vpk pack` and then
  `verify-package.ps1` (all files present, the feed matches the files, the exe in the Portable
  zip is the one that was verified; on Linux, the AppImage is an ELF file and the full package
  holds the same AppImage).
- The feed, `releases.win-<arch>.json`, is cut down to this version's packages. vpk's own copy
  also lists the previous full package it downloaded, but Velopack's docs want one full and one
  delta package per release, and `vpk upload` builds the feed from the current build's assets
  only, so the feed we upload does the same.
- `publish` attests, checksums and uploads to the **draft**. It uses `gh release upload`, not
  `vpk upload github`, because vpk refuses a draft that already holds the feed (that would
  break `recover`). Nothing in the workflow publishes except `go-live`.

### How updates reach users

The app reads the feeds of the 10 newest **published, non-pre-release** releases on GitHub.
A draft is invisible to it, so `go-live` stays the only thing that decides what users get.
A pre-release (a version with a suffix such as `-beta.1`) is not offered to anyone. Releases
without a feed (v0.2.x) are skipped. The first Velopack release has no delta; later ones do.

### Bumping vpk

`vpk` is pinned in `pack-velopack.ps1` (`$VpkVersion`) and must equal the `velopack` crate in
`Cargo.lock`; the script fails otherwise, and also when the crate is missing (only a `-NoDownload`
dry run tolerates that). Dependabot bumps the crate, so bump `$VpkVersion` in
the same PR. The CI jobs `build (windows-x64, release)`, `build (windows-arm64, release)` and
`build (linux-x64, release)` run
the same script as a dry run (`-NoDownload`), so a mismatch or a vpk problem shows up on the PR.

### Packing locally

Needs the .NET SDK and a release build:

```powershell
cargo build --release -p pr-monitor
./.github/scripts/pack-velopack.ps1 -Rid win-x64 -Exe target/release/pr-monitor.exe -OutDir pack-out -NoDownload
```

On Linux, pass the Linux rid, the binary and `-Version` (a Linux binary has no ProductVersion):
`pwsh ./.github/scripts/pack-velopack.ps1 -Rid linux-x64 -Version 0.3.0 -Exe target/release/pr-monitor -OutDir pack-out -NoDownload`.
CI runs this as the `build (linux-x64, release)` dry run.

### Linux specifics

- Updates stage in `/var/tmp`. If the AppImage sits in a privileged folder, applying an update
  prompts through `pkexec`. The first update after an install is always a full download (no fast
  hooks, no delta from the unpacked install).
- There is no uninstall hook: delete the AppImage. `pr-monitor --unregister` removes the autostart
  entry first.
- Autostart entries use `$APPIMAGE` (when `$APPDIR` confirms the running binary is inside that
  AppImage), because `current_exe()` inside an AppImage points into a temporary mount. The path is
  quoted for the `Exec=` line, so spaces are fine.
- The `prmonitor://` link handler is Windows-only for now; nothing registers it on Linux.
- Velopack's own log is `/tmp/velopack_AndrewKorchemniy.PRMonitor.log`; the app's logs stay in
  its data folder.
- The tray icon needs a StatusNotifier host (GNOME needs an AppIndicator extension).

### winget

The first submission of `AndrewKorchemniy.PRMonitor` to winget-pkgs is manual (the `winget`
job only updates an existing package): run `wingetcreate new` with the `Setup.exe` URLs and set
`InstallerType: exe`, `Scope: user` and `InstallerSwitches` `Silent` and `SilentWithProgress`
to `--silent`. Install it once from a local manifest before turning `WINGET_ENABLED` on. The
job then picks `AndrewKorchemniy.PRMonitor-win-<arch>-Setup.exe` from each release.
## Manual release (no App)

1. In a PR, bump `version` in `crates/pr-monitor/Cargo.toml` (line 3) and run
   `cargo build --workspace` so `Cargo.lock` picks up the new version. CI builds with
   `--locked`, so a stale lockfile fails the PR. Merge it.
2. Run the workflow on `main` with `mode=dry-run`. Download the artifact, run the exe and read
   the job summary.
3. Run it on `main` with `mode=release-current`.
4. When the publish job is done, check the draft release on GitHub, then approve `go-live`.

Release notes come from `.github/scripts/release-notes.sh` plus the footer from
`.github/scripts/release-footer.sh` (how to install and verify, and a note that builds are unsigned).
For each pull request merged since the last published release, the notes show:
- its title, as a heading;
- its description's "What and why" (the test notes, checklist and HTML comments are left out);
- the titles of the commits it was squashed from, in a collapsed list.
Preview them before a release with
`PREV_TAG=v0.2.0 bash .github/scripts/release-notes.sh AndrewKorchemniy/pr-monitor <sha> v0.2.1`.
`CHANGELOG.md` is only updated on the release-plz path.

## release-plz with a GitHub App (optional)

release-plz needs a GitHub App because pull requests opened with the default `GITHUB_TOKEN`
do not trigger other workflows, so CI (and the required `ci-ok` check) would never run on the
release PR.

1. Create a GitHub App with Contents (read and write) and Pull requests (read and write)
   permissions, and install it on the repository.
2. Add the repository variable `RELEASE_APP_CLIENT_ID` (the App's client ID) and the secret
   `RELEASE_APP_PRIVATE_KEY` (its private key).

Then on every push to `main`:

- The `release-plz PR` job opens or updates a PR titled `chore: release v<version>` with the
  `release` label, the version bump and the `CHANGELOG.md` entry. Only `feat`, `fix`, `perf`
  and breaking commits open one (`release_commits` in `release-plz.toml`).
- When that PR is merged, the `release-plz release` job creates the `v<version>` tag and a
  draft release, and the build, publish and `go-live` jobs follow as in `release-current`.

Run a dry run on `main` before merging the release PR. release-plz reads versions from git
tags (`git_only = true`), so it continues from the last `v*` tag, including one created by
the manual path. Do not run `release-current` for a version that a release-plz PR is also
about to release.

## When something fails

- **Before `go-live`:** the release is still a draft. `mode=recover` with the tag rebuilds the
  commit the release was created from and re-uploads the assets with `--clobber`. Run it from
  `main`, so a fix to the workflow itself, or a retry after a flaky runner, takes effect. A fix
  to the code is not picked up, because recover does not build newer commits; for that,
  bump the version and release again.
- **After `go-live`:** the tag exists and can never be deleted or moved (the `tags.json`
  ruleset). Bump the patch version and release again.
- **`release-current` says the release exists:** bump the version; a version is released at
  most once.
- **An install or update misbehaves on a user's machine:** `Update.exe` logs to
  `%LocalAppData%\velopack\velopack_AndrewKorchemniy.PRMonitor.log`; the app's own logs stay in
  `%LOCALAPPDATA%\PRMonitor\logs`.
- **Two copies of the app:** a Portable copy and an installed copy share the data folder
  `%LOCALAPPDATA%\PRMonitor` and the single-instance key, so only one of them runs at a time.

## Verifying a release

```powershell
gh release download v0.1.0 -R AndrewKorchemniy/pr-monitor
gh attestation verify AndrewKorchemniy.PRMonitor-win-x64-Setup.exe -R AndrewKorchemniy/pr-monitor
gh attestation verify AndrewKorchemniy.PRMonitor-win-x64-Setup.exe -R AndrewKorchemniy/pr-monitor --predicate-type https://cyclonedx.org/bom
Get-FileHash -Algorithm SHA256 AndrewKorchemniy.PRMonitor-win-*   # compare with SHA256SUMS.txt
```

Each build job also runs `.github/scripts/verify-exe.ps1` on the release exe: PE machine and
GUI subsystem, the 24 MiB size budget, no dynamic CRT, `ProductVersion` equal to the release
version, and a headless `--self-test`. The arm64 exe is checked on a native arm64 runner.
The `package` jobs then run `verify-package.ps1` on each package (see above).

## Settings

Variables and secrets that switch optional jobs on (nothing is needed for a plain release):

| Name | Kind | Effect |
|---|---|---|
| `WINGET_ENABLED` | variable | `true` enables the `winget` job, which submits the release to winget as `AndrewKorchemniy.PRMonitor`. It is skipped in dry runs. |
| `WINGET_TOKEN` | secret | Classic PAT with `public_repo` and `workflow` scopes, used by the winget job. |
| `RELEASE_APP_CLIENT_ID` | variable | Enables the release-plz PR job (see above). |
| `RELEASE_APP_PRIVATE_KEY` | secret | The App's private key. |

The `go-live` and `winget` jobs run in the `release` environment, so put the winget secret and
variable there.
