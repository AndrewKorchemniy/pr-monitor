# Security policy

## Supported versions
Only the latest release receives fixes.

## Reporting a vulnerability
Report privately via GitHub: https://github.com/AndrewKorchemniy/pr-monitor/security/advisories/new
Please do not open public issues for security problems. Include the version, Windows build, sign-in method
and reproduction steps, and never include real tokens. Expect an acknowledgement within 3 business days and a
fix or mitigation plan within 30 days for high-severity issues. Credit is given unless you prefer otherwise.

## Scope
- Handling of Azure CLI access tokens, PATs and Entra refresh tokens (Windows Credential Manager / DPAPI)
- Files in `%LOCALAPPDATA%\PRMonitor` (config.toml, state.json, logs)
- The `prmonitor://` protocol handler, toast activation and the single-instance named pipe
- Network requests to Azure DevOps / Microsoft Entra and proxy (PAC) handling
- Release integrity (attestations, checksums, signing)

Out of scope: vulnerabilities in Azure DevOps itself, the Azure CLI, or Windows.

## Verifying downloads
```powershell
gh attestation verify pr-monitor-windows-x64.exe -R AndrewKorchemniy/pr-monitor
```
Compare `Get-FileHash -Algorithm SHA256` against `SHA256SUMS.txt` from the same release.
