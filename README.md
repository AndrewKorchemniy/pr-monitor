# PR Monitor

[![CI](https://github.com/AndrewKorchemniy/pr-monitor/actions/workflows/ci.yml/badge.svg)](https://github.com/AndrewKorchemniy/pr-monitor/actions/workflows/ci.yml)

A Windows tray app that watches your team's Azure DevOps pull requests and shows whose move
each one is.

> [!NOTE]
> Early preview. If something breaks,
> [open an issue](https://github.com/AndrewKorchemniy/pr-monitor/issues/new/choose) and attach
> the relevant lines from `%LOCALAPPDATA%\pr-monitor\logs`.

- **One status per PR:** *Needs review*, *Active comments* or *Reviewed*, based on the
  reviewers whose votes count for your team.
- **Many orgs and repos.** Choose projects or single repositories, the people whose PRs you
  track (empty tracks everyone in those repositories) and the reviewers whose votes count.
- **Notifications** for new PRs, comments, votes, re-reviews and pushes, grouped per PR and
  capped per refresh.
- **Lives in the tray.** It refreshes every 5 minutes by default. `Ctrl+K` opens the command
  palette; `?` lists every shortcut.
- **Updates itself.** Installer or portable zip; no admin rights.

## Download

Get the package for your architecture (x64 or arm64) from the
[latest release](https://github.com/AndrewKorchemniy/pr-monitor/releases/latest):

- **Installer:** `pr-monitor-win-x64-Setup.exe` (or `-win-arm64`). Installs for your user into
  `%LOCALAPPDATA%\pr-monitor`, with a Start menu entry and an Add/Remove Programs entry.
- **Portable zip:** `pr-monitor-win-x64-Portable.zip` (or `-win-arm64`). Unzip anywhere, run
  `PR Monitor.exe` and keep the folder together: it updates itself in place.

Direct links:
[Setup x64](https://github.com/AndrewKorchemniy/pr-monitor/releases/latest/download/pr-monitor-win-x64-Setup.exe),
[Setup arm64](https://github.com/AndrewKorchemniy/pr-monitor/releases/latest/download/pr-monitor-win-arm64-Setup.exe),
[Portable x64](https://github.com/AndrewKorchemniy/pr-monitor/releases/latest/download/pr-monitor-win-x64-Portable.zip),
[Portable arm64](https://github.com/AndrewKorchemniy/pr-monitor/releases/latest/download/pr-monitor-win-arm64-Portable.zip).

Supported: Windows 10 1809 or later (x64) and Windows 11 (arm64).

Releases are built by GitHub Actions and are not code-signed. Verify a download (the same works
for the Portable zip):

```powershell
gh attestation verify pr-monitor-win-x64-Setup.exe -R AndrewKorchemniy/pr-monitor
gh attestation verify pr-monitor-win-x64-Setup.exe -R AndrewKorchemniy/pr-monitor --predicate-type https://cyclonedx.org/bom
Get-FileHash -Algorithm SHA256 pr-monitor-win-x64-Setup.exe   # compare with SHA256SUMS.txt
```

The first command checks build provenance, the second the CycloneDX SBOM.

### Linux (preview)

Linux builds are tested in CI but used less than the Windows ones. Download
`pr-monitor-linux-x64.AppImage` (or `-linux-arm64`) from the
[latest release](https://github.com/AndrewKorchemniy/pr-monitor/releases/latest), then:

```sh
chmod +x pr-monitor-linux-x64.AppImage
./pr-monitor-linux-x64.AppImage
```

- **Updates** install themselves. Keep the AppImage in a folder you can write to (for example
  `~/Applications`); otherwise an update asks for a password.
- **Libraries:** libxkbcommon, X11 or Wayland, and GL (Mesa or a vendor driver). If FUSE is
  missing, run it with `--appimage-extract-and-run`.
- **Tray icon:** needs a StatusNotifier host (GNOME needs the AppIndicator extension).
- **Launch at login** points at the AppImage file; if you move it, turn the setting off and on.
- Needs glibc 2.35 or newer (Ubuntu 22.04 and later).
- The `prmonitor://` link handler is Windows-only.
- Not code-signed. Verify with `gh attestation verify <file> -R AndrewKorchemniy/pr-monitor`.

## Signing in

Automatic sign-in (the default) tries your Azure CLI session first, then your organization's app
(if you set one up), then a saved personal access token. It remembers what worked for each
organization. To choose a method yourself, use Settings › Organizations › **Change sign-in**:

- **Sign in with Microsoft** runs `az login` for the organization's tenant. **Sign in with the
  browser instead** does the same, forcing the system browser instead of Windows' account
  broker. Both need the Azure CLI (`az`).
- **Personal access token.** It needs Code (Read), Project and Team (Read), Graph (Read) and
  Identity (Read). The token is kept in Windows Credential Manager.
- **Your organization's app (advanced).** An Entra public-client app registration with an
  `http://localhost` redirect, which your admin provides. Enter its Client ID (and tenant)
  under Advanced.

## Your data

Settings (`config.toml`, `keymap.toml`, `themes`) live in `%APPDATA%\pr-monitor`. Cached results,
window state, avatars, logs and the single-instance lock live in `%LOCALAPPDATA%\pr-monitor`
(logs in `logs`). `--home <dir>` or the `PRMONITOR_HOME` environment variable keeps everything in
one folder instead; `--home` wins. A copy started that way never updates itself. Logs never
contain tokens. PR Monitor talks only to Azure DevOps, Microsoft sign-in and GitHub (for update
checks; turn those off in Settings › General).

Uninstalling from Add/Remove Programs removes the program, its Windows entries and saved
sign-ins; your settings in `%APPDATA%\pr-monitor` stay. `pr-monitor --unregister --purge` removes
what PR Monitor added to Windows and both data folders, and keeps the installed program.

## Command line

```text
pr-monitor [--minimized] [--activate <prmonitor://...>] [--renderer auto|glow|wgpu]
           [--home <dir>] [--debug] [--self-test] [--unregister [--purge]] [--version] [--help]
```

`pr-monitor --help` describes each flag.

## Troubleshooting

- **Sign-in keeps failing.** Run `az login` in a terminal and check that
  `az account get-access-token --resource 499b84ac-1321-427f-aa17-267ca6975798` works. If your
  organization blocks the Azure CLI, use a personal access token under Settings › Organizations ›
  Change sign-in.
- **No pull requests show up.** Check Settings › Organizations: the organization must be enabled
  and signed in, with projects or repositories chosen.
- **The window is blank or flickers, especially over Remote Desktop.** Start PR Monitor with
  `--renderer glow`, or choose OpenGL under Settings › Appearance.
- **Something else.** Logs are in `%LOCALAPPDATA%\pr-monitor\logs`. `pr-monitor --debug` also
  logs to the console. Attach the latest log when you
  [report a problem](https://github.com/AndrewKorchemniy/pr-monitor/issues/new/choose).

## Building from source

Needs Rust (pinned in `rust-toolchain.toml`) and, on Windows, the MSVC build tools.

```powershell
cargo build --release -p pr-monitor        # target/release/pr-monitor.exe
cargo lint                                 # clippy, warnings are errors
cargo t                                    # tests (cargo-nextest)
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/THEMING.md](docs/THEMING.md),
[CONTRIBUTING.md](CONTRIBUTING.md) and [docs/RELEASING.md](docs/RELEASING.md).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

The bundled fonts are under the SIL Open Font License 1.1
([Next](crates/prm-ui/assets/fonts/OFL-AtkinsonHyperlegibleNext.txt),
[Mono](crates/prm-ui/assets/fonts/OFL-AtkinsonHyperlegibleMono.txt)):

- Atkinson Hyperlegible Next: Copyright 2020-2024 The Atkinson Hyperlegible Next Project
  Authors (https://github.com/googlefonts/atkinson-hyperlegible-next)
- Atkinson Hyperlegible Mono: Copyright 2020-2024 The Atkinson Hyperlegible Mono Project
  Authors (https://github.com/googlefonts/atkinson-hyperlegible-next-mono)

<sub>Microsoft, Windows, Azure and Azure DevOps are trademarks of the Microsoft group of
companies. PR Monitor is an independent project and is not affiliated with, endorsed by or
sponsored by Microsoft.</sub>
