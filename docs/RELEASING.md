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
   the signing and winget secrets also live in this environment.
6. Optional: turn on **Immutable releases**. Published releases and their assets then can no
   longer change, which is why assets are uploaded while the release is still a draft.
7. Run the workflow in `dry-run` mode and check the `release-dry-run` artifact.
8. Run it in `release-current` mode to create v0.1.0.
9. Approve the `go-live` job.

## Workflow modes

Run **Actions › Release › Run workflow**, or `gh workflow run release.yml -f mode=<mode>`.

| Mode | What it does |
|---|---|
| `dry-run` (default) | Builds and verifies both exes from the selected commit, makes the zips and `SHA256SUMS.txt`, and uploads them as the `release-dry-run` workflow artifact (kept 7 days). Creates no release, no tag and no attestations. |
| `release-current` | Must run on `main`. Reads the version from `crates/pr-monitor/Cargo.toml` and fails if a release `v<version>` already exists. Otherwise creates a draft release `v<version>` targeting the current commit, builds, signs (if configured), uploads assets and attestations to the draft, then waits for `go-live` approval. |
| `recover` | Takes `tag` (for example `v0.3.0`). Rebuilds and re-uploads the assets of an existing **draft** release for that tag, then waits for `go-live` approval. |

A draft release does not create the git tag. GitHub creates `v<version>` on the target commit
when `go-live` publishes the release.

`go-live` runs in the `release` environment and only marks the draft as published: as the
latest release, or as a pre-release when the version has a suffix such as `-alpha.1` or
`-beta.2`. Nothing becomes public until a reviewer approves it.

## Manual release (no App)

1. In a PR, bump `version` in `crates/pr-monitor/Cargo.toml` (line 3) and run
   `cargo build --workspace` so `Cargo.lock` picks up the new version. CI builds with
   `--locked`, so a stale lockfile fails the PR. Merge it.
2. Run the workflow on `main` with `mode=dry-run`. Download the artifact, run the exe and read
   the job summary.
3. Run it on `main` with `mode=release-current`.
4. When the publish job is done, check the draft release on GitHub, then approve `go-live`.

Release notes come from GitHub's generated notes plus the unsigned-build and verify lines.
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

## Verifying a release

```powershell
gh release download v0.1.0 -R AndrewKorchemniy/pr-monitor
gh attestation verify pr-monitor-windows-x64.exe -R AndrewKorchemniy/pr-monitor
gh attestation verify pr-monitor-windows-x64.exe -R AndrewKorchemniy/pr-monitor --predicate-type https://cyclonedx.org/bom
Get-FileHash -Algorithm SHA256 pr-monitor-windows-*.exe   # compare with SHA256SUMS.txt
```

Each build job also runs `.github/scripts/verify-exe.ps1` on the release exe: PE machine and
GUI subsystem, the 20 MiB size budget, no dynamic CRT, `ProductVersion` equal to the release
version, and a headless `--self-test`. The arm64 exe is checked on a native arm64 runner.

## Settings

Variables and secrets that switch optional jobs on:

| Name | Kind | Effect |
|---|---|---|
| `SIGNING_PROVIDER` | variable | `signpath` or `azure` enables the `sign` job. Unset or `none` ships unsigned exes. |
| `SIGNPATH_API_TOKEN` | secret | SignPath API token (`signpath`). |
| `SIGNPATH_ORGANIZATION_ID` | variable | SignPath organization (`signpath`). |
| `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_SUBSCRIPTION_ID` | variables | OIDC login for Azure Artifact Signing (`azure`). |
| `AZURE_SIGNING_ENDPOINT`, `AZURE_SIGNING_ACCOUNT`, `AZURE_CERT_PROFILE` | variables | Signing account and certificate profile (`azure`). |
| `WINGET_ENABLED` | variable | `true` enables the `winget` job, which submits the release to winget as `AndrewKorchemniy.PRMonitor`. It is skipped in dry runs. |
| `WINGET_TOKEN` | secret | Classic PAT with `public_repo` and `workflow` scopes, used by the winget job. |
| `RELEASE_APP_CLIENT_ID` | variable | Enables the release-plz PR job (see above). |
| `RELEASE_APP_PRIVATE_KEY` | secret | The App's private key. |

The signing and winget jobs run in the `release` environment, so put their secrets and
variables there.
