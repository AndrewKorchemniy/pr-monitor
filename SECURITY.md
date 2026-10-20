# Security policy

## Supported versions
Only the latest release receives fixes.

## Reporting a vulnerability
Report privately via GitHub: https://github.com/AndrewKorchemniy/pr-monitor/security/advisories/new
Please do not open public issues for security problems. Include the version, Windows build, sign-in method
and reproduction steps, and never include real tokens. Logs (`%LOCALAPPDATA%\PRMonitor\logs`) redact
tokens but not organization, project or repository names, so redact those before attaching logs.
Expect an acknowledgement within 3 business days and a fix or mitigation plan within 30 days for
high-severity issues. Credit is given unless you prefer otherwise.

## Scope
- Handling of Azure CLI access tokens, PATs and Entra refresh tokens (Windows Credential Manager / DPAPI)
- Files in `%LOCALAPPDATA%\PRMonitor` (config.toml, state.json, logs)
- The `prmonitor://` protocol handler, toast activation and the single-instance named pipe
- Network requests to Azure DevOps / Microsoft Entra and proxy (PAC) handling
- Release integrity (attestations, checksums, signing)

Out of scope: vulnerabilities in Azure DevOps itself, the Azure CLI, or Windows.

## Known limitations
- Release binaries may not be Authenticode-signed. Verify them with the attestations and checksums below.
- A PAT or refresh token stored in Windows Credential Manager can be read by any other process running as
  the same Windows user. This is how Credential Manager works; PR Monitor cannot prevent it.

## Verifying downloads
```powershell
gh attestation verify pr-monitor-windows-x64.exe -R AndrewKorchemniy/pr-monitor
gh attestation verify pr-monitor-windows-x64.exe -R AndrewKorchemniy/pr-monitor --predicate-type https://cyclonedx.org/bom
```
The first command checks build provenance, the second the CycloneDX SBOM attestation. Compare
`Get-FileHash -Algorithm SHA256` against `SHA256SUMS.txt` from the same release.
