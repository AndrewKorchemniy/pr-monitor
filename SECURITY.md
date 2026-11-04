# Security policy

## Supported versions

Only the latest release receives fixes.

## Reporting a vulnerability

Report privately through
[GitHub security advisories](https://github.com/AndrewKorchemniy/pr-monitor/security/advisories/new).
Do not open a public issue. Include the version, Windows build, sign-in method and reproduction
steps, and never include real tokens. Logs (`%LOCALAPPDATA%\pr-monitor\logs`) redact tokens but
not organization, project or repository names, so redact those before attaching them.

Expect an acknowledgement within 3 business days, and a fix or mitigation plan within 30 days for
high-severity issues. Credit is given unless you prefer otherwise.

## Scope

- Handling of Azure CLI access tokens, PATs and Entra refresh tokens (Windows Credential Manager
  and DPAPI)
- Files in `%APPDATA%\pr-monitor` and `%LOCALAPPDATA%\pr-monitor` (`config.toml`, `state.json`,
  logs)
- The `prmonitor://` protocol handler, toast activation and the single-instance named pipe
- Network requests to Azure DevOps and Microsoft Entra, and proxy (PAC) handling
- Release integrity (attestations, checksums)

Out of scope: vulnerabilities in Azure DevOps itself, the Azure CLI, or Windows.

## Known limitations

A PAT or refresh token stored in Windows Credential Manager can be read by any other process
running as the same Windows user. This is how Credential Manager works; PR Monitor cannot prevent
it.

## Verifying downloads

See [Download in the README](README.md#download) for the `gh attestation verify` commands and
how to compare the SHA-256 checksum with `SHA256SUMS.txt`.
