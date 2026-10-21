# PR Monitor

[![CI](https://github.com/AndrewKorchemniy/pr-monitor/actions/workflows/ci.yml/badge.svg)](https://github.com/AndrewKorchemniy/pr-monitor/actions/workflows/ci.yml)

A small, fast Windows tray app that watches your team's Azure DevOps pull requests and
tells you whose move it is.

> [!NOTE]
> PR Monitor is an early preview. Expect rough edges. If something breaks,
> [open an issue](https://github.com/AndrewKorchemniy/pr-monitor/issues/new/choose) and attach
> the relevant lines from the logs folder (`%LOCALAPPDATA%\PRMonitor\logs`).

- **One status per PR:** *Needs review*, *Active comments* or *Reviewed*, based on the
  reviewers whose votes count for your team.
- **Many orgs and repos at once.** Choose projects or single repositories, the people whose
  PRs you track and the reviewers whose votes count.
- **Notifications that are worth reading:** new PRs, new comments, votes, re-reviews and
  pushes, grouped per PR and capped per refresh.
- **Lives in the tray.** It refreshes every few minutes (5 by default) and uses no CPU while
  idle. Press `Ctrl+K` for the command palette and `?` for every shortcut.
- **One portable exe** of about 19 MB, with no installer and no runtime to install.

## Download

Get `pr-monitor-windows-x64.exe` (or `-arm64`) from the
[latest release](https://github.com/AndrewKorchemniy/pr-monitor/releases/latest) and run it.
The zip version also includes the licenses. Direct links to the latest build:

- [pr-monitor-windows-x64.exe](https://github.com/AndrewKorchemniy/pr-monitor/releases/latest/download/pr-monitor-windows-x64.exe)
- [pr-monitor-windows-arm64.exe](https://github.com/AndrewKorchemniy/pr-monitor/releases/latest/download/pr-monitor-windows-arm64.exe)

Builds are not code-signed yet, so Windows SmartScreen may warn on first launch
(**More info › Run anyway**). Every release is built by GitHub Actions from a tagged
commit, and you can verify that yourself:

```powershell
gh attestation verify pr-monitor-windows-x64.exe -R AndrewKorchemniy/pr-monitor
gh attestation verify pr-monitor-windows-x64.exe -R AndrewKorchemniy/pr-monitor --predicate-type https://cyclonedx.org/bom
Get-FileHash -Algorithm SHA256 pr-monitor-windows-x64.exe   # compare with SHA256SUMS.txt
```

The first command checks the build provenance, the second the attached CycloneDX SBOM.

Supported: Windows 10 1809 or later (x64) and Windows 11 (arm64).

## Signing in

PR Monitor tries these in order and remembers what worked for each organization:

1. **Azure CLI.** If `az login` already works on your machine, there is nothing to set up.
2. **Sign in with Microsoft.** This runs `az login` for the organization's tenant.
3. **Personal access token.** It needs Code (Read), Project and Team (Read), Graph (Read) and
   Identity (Read). The token is kept in Windows Credential Manager.
4. **Your organization's app (advanced).** This uses an Entra app registration that your
   admin provides.

## Your data

Settings, cached results and logs live in `%LOCALAPPDATA%\PRMonitor` (logs in its `logs`
subfolder). To keep them somewhere else, pass `--home <dir>` or set the `PRMONITOR_HOME`
environment variable; `--home` wins if both are set. Logs are redacted: tokens never reach
them. PR Monitor talks only to Azure DevOps and Microsoft sign-in.
`pr-monitor --unregister --purge` removes everything it added to Windows, including the data
folder.

## Command line

```text
pr-monitor [--minimized] [--activate <prmonitor://...>] [--renderer auto|glow|wgpu]
           [--home <dir>] [--debug] [--self-test] [--unregister [--purge]] [--version]
```

`pr-monitor --help` describes each flag.

## Troubleshooting

- **Sign-in keeps failing.** Run `az login` in a terminal and check that
  `az account get-access-token --resource 499b84ac-1321-427f-aa17-267ca6975798` works. If your
  organization blocks the Azure CLI, use a personal access token from Settings ›
  Organizations › Change sign-in.
- **No pull requests show up.** Check Settings › Organizations: the organization must be
  watched, signed in, and have repositories and people chosen.
- **The window is blank or flickers, especially over Remote Desktop.** Start PR Monitor with
  `--renderer glow`, or choose OpenGL under Settings › Appearance.
- **Something else.** Logs are in `%LOCALAPPDATA%\PRMonitor\logs` (redacted: no tokens).
  `pr-monitor --debug` logs more detail to the console. Attach the latest log when you
  [report a problem](https://github.com/AndrewKorchemniy/pr-monitor/issues/new/choose).

## Building from source

You need Rust (the toolchain is pinned in `rust-toolchain.toml`) and, on Windows, the MSVC
build tools.

```powershell
cargo build --release -p pr-monitor        # target/release/pr-monitor.exe
cargo lint                                 # clippy, warnings are errors
cargo t                                    # tests (cargo-nextest)
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how it fits together,
[docs/THEMING.md](docs/THEMING.md) for writing themes, [CONTRIBUTING.md](CONTRIBUTING.md)
before opening a pull request and [docs/RELEASING.md](docs/RELEASING.md) for how releases
are made.

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
