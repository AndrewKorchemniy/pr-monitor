# Helpers shared by the release scripts. Dot-source it: `. "$PSScriptRoot/common.ps1"`.

# The Velopack package id; also prm_domain::app::AUMID (shortcuts and toasts depend on that).
$PackId = 'AndrewKorchemniy.PRMonitor'

function Write-Summary([string] $Line) {
    Write-Host $Line
    if ($env:GITHUB_STEP_SUMMARY) { Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Value $Line }
}

# Runs a native command and throws when it exits non-zero ($ErrorActionPreference does not cover that).
function Invoke-Native([string] $Command, [string[]] $Arguments) {
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Command exited with $LASTEXITCODE" }
}
