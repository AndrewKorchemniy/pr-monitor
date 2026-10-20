# Contributing

Thanks for helping. This page covers what you need to build, test and send a change.

## Prerequisites

- **Rust via rustup.** The toolchain is pinned in `rust-toolchain.toml`; rustup installs it
  the first time you run `cargo` in the repo.
- **Windows:** Visual Studio Build Tools with the C++ build tools (VCTools) workload. The
  release check in `.github/scripts/verify-exe.ps1` also uses its `dumpbin.exe`.
- **Linux:** the system packages listed in `.github/actions/linux-deps/action.yml`.
- **Cargo tools:**

  ```powershell
  cargo install --locked cargo-nextest cargo-deny cargo-shear typos-cli cargo-insta
  ```

## Everyday commands

The aliases live in `.cargo/config.toml`, and CI uses the same ones.

```powershell
cargo app              # run the app (dev-ui profile); pass app flags after the alias
cargo lint             # clippy on the whole workspace, warnings are errors
cargo t                # tests with cargo-nextest
cargo test --workspace --doc --all-features --locked   # doctests (nextest does not run them)
cargo insta review     # accept or reject changed snapshots
cargo fmt --all
typos
cargo shear
```

## Rules

- [docs/RUST_STYLE.md](docs/RUST_STYLE.md) is binding. Read it before your first change.
- Never set `RUSTFLAGS`, locally or in CI. It replaces the `+crt-static` flags in
  `.cargo/config.toml`.
- Tests are hermetic: no real network (use wiremock), no credential store, no registry, no
  toasts, no wall clock, no sleeps. See RUST_STYLE section 10.
- `unsafe` is allowed only in `crates/prm-platform`. See RUST_STYLE section 11.

## Pull requests

- The PR title must be a [Conventional Commit](https://www.conventionalcommits.org/), for
  example `fix(ui): keep the selection after refresh`. PRs are squash-merged, so the title
  becomes the commit message and feeds the changelog.
- Fill in the PR template: what changed, why and how you tested it. Attach dark and light
  screenshots for UI changes.

## Dependencies

- A new dependency's license must be on the allowlist in both `deny.toml` and `about.toml`.
- Note the binary-size change in the PR. The release exe must stay under the 20 MiB budget
  checked by `verify-exe.ps1`.
- Declare the dependency in `[workspace.dependencies]` with minimal features.

## License

Unless you explicitly state otherwise, any contribution you intentionally submit for
inclusion in this project, as defined in the Apache-2.0 license, shall be dual-licensed as
MIT OR Apache-2.0, without any additional terms or conditions.
