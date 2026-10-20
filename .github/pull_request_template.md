<!-- The PR title becomes the squash commit and drives the changelog. Use a Conventional Commit:
feat|fix|perf|refactor|docs|test|build|ci|chore|revert|style, optional (scope), lowercase subject,
`!` for breaking changes, e.g. `feat(ui)!: replace status column`. -->

## What and why

## How it was tested

## Checklist
- [ ] `cargo fmt --all` and `cargo lint` are clean
- [ ] `cargo t` passes; new logic in prm-domain / prm-service / prm-ado / prm-auth has unit tests (fakes, wiremock, insta)
- [ ] No test touches the network (other than wiremock on loopback), the credential store, the registry or toasts
- [ ] UI changes: before/after screenshots (dark + light) attached
- [ ] New dependency: justified, license on the deny.toml allowlist, binary-size delta noted
