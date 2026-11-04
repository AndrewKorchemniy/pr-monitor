# Helpers shared by the release scripts. Dot-source it: `. "$PSScriptRoot/common.ps1"`.

# The Velopack package id; it names the release files and the install folder.
$PackId = 'pr-monitor'

# The Windows app identity for shortcuts and toasts. Must equal prm_domain::app::AUMID.
$Aumid = 'AndrewKorchemniy.PRMonitor'

# 'windows' or 'linux' for a Velopack runtime id such as win-x64 or linux-arm64.
function Get-Platform([string] $Rid) {
    if ($Rid -like 'win-*') { 'windows' } else { 'linux' }
}

function Write-Summary([string] $Line) {
    Write-Host $Line
    if ($env:GITHUB_STEP_SUMMARY) { Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Value $Line }
}

# Runs a native command and throws when it exits non-zero ($ErrorActionPreference does not cover that).
function Invoke-Native([string] $Command, [string[]] $Arguments) {
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Command exited with $LASTEXITCODE" }
}
