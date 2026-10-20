# AGENTS.md

Guidance for coding agents working in this repository. Humans should read
[CONTRIBUTING.md](CONTRIBUTING.md); this file points to the same rules.

## What this is

PR Monitor is a Windows-first tray app, written in Rust with egui, that watches Azure DevOps
pull requests and shows whose move each one is. It ships as one statically linked exe for
x64 and arm64. The workspace has nine crates; pure logic sits at the bottom
(`prm-domain`, `prm-theme`) and the binary (`crates/pr-monitor`) at the top.

## Read first

- [docs/RUST_STYLE.md](docs/RUST_STYLE.md): binding style rules. Follow them exactly.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): crates, dependency direction, threads and
  data flow.
- [docs/THEMING.md](docs/THEMING.md): theme files and tokens, when touching `prm-theme` or UI
  colours.
- [docs/RELEASING.md](docs/RELEASING.md): only when touching `.github/workflows/release.yml`
  or `release-plz.toml`.

## Golden rules

These summarize RUST_STYLE; the linked section is the authority.

- Tests live in `<module>/tests.rs` behind `#[cfg(test)] mod tests;`, never inline. No
  `mod.rs`. (section 1)
- Keep crate dependencies one-way; `prm-domain` does no IO, async, egui or OS calls.
  (section 1.7)
- Functions stay within 60 lines, 5 parameters and 3 nesting levels. (section 5)
- No `unwrap`/`expect` outside tests. Do not use `_` match arms on this workspace's own enums.
  (sections 5 and 7)
- Suppress lints with `#[expect(lint, reason = "...")]`, not a bare `#[allow]`. (section 11.1)
- `unsafe` only in `crates/prm-platform/src/win/*`. (section 11.2)
- Tests are deterministic: no network except wiremock on loopback, no wall clock, no sleeps,
  seeded randomness only. (section 10.3)
- New dependencies go in `[workspace.dependencies]` and must pass `deny.toml` and `about.toml`.
  (section 11.3)
- Never set `RUSTFLAGS`. It replaces the `+crt-static` flags in `.cargo/config.toml`.
- No `println!`, `eprintln!` or `dbg!`; log with `tracing`. (section 9)

## Before you finish

Run these and fix everything they report:

```powershell
cargo fmt --all --check
cargo lint
cargo t
cargo test --workspace --doc --all-features --locked
typos
cargo deny --all-features --locked check bans licenses sources
cargo shear
```

`cargo lint` and `cargo t` are aliases from `.cargo/config.toml`; `cargo t` needs
cargo-nextest. If nextest is not installed, use
`cargo test --workspace --all-features --locked` instead.

If you changed the binary, `prm-platform`, dependencies or build settings, also build and
check the release exe:

```powershell
cargo build --release --locked -p pr-monitor
./.github/scripts/verify-exe.ps1 -Exe target/release/pr-monitor.exe -Arch x64
```

Use `-Arch arm64` on an arm64 machine. The script needs PowerShell 7.4 and the Visual Studio
C++ build tools.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/):
`feat|fix|perf|refactor|docs|test|build|ci|chore|revert|style`, an optional `(scope)`, and a
lowercase subject, for example `fix(ui): keep the selection after refresh`. Add `!` for a
breaking change. PRs are squash-merged, so the PR title becomes the commit and drives the
changelog.
