#Requires -Version 7.4
<#
.SYNOPSIS
  Velopack package gate: the files pack-velopack.ps1 produced are complete, consistent and carry the app exe.
  It does not run anything: --self-test already ran on the app exe (verify-exe.ps1) and pack copies it unchanged.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Dir,
    [Parameter(Mandatory)][ValidateSet('win-x64', 'win-arm64')][string] $Rid,
    [Parameter(Mandatory)][string] $Version,
    # The app exe that was packed.
    [Parameter(Mandatory)][string] $Exe,
    # The package was made without THIRD-PARTY-LICENSES.html (CI dry runs do not build the notices).
    [switch] $NoNotices
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/common.ps1"
Add-Type -AssemblyName System.IO.Compression.FileSystem

$setup = Join-Path $Dir "$PackId-$Rid-Setup.exe"
$portable = Join-Path $Dir "$PackId-$Rid-Portable.zip"
$full = Join-Path $Dir "$PackId-$Version-$Rid-full.nupkg"
foreach ($f in $setup, $portable, $full) {
    if (-not (Test-Path -LiteralPath $f)) { throw "missing $(Split-Path -Leaf $f)" }
}

# 1. The feed names this version's files only, and each file is there with the size and hash it states.
$feed = Get-Content -Raw (Join-Path $Dir "releases.$Rid.json") | ConvertFrom-Json
$entries = @($feed.Assets)
if (-not ($entries | Where-Object Type -eq 'Full')) { throw "releases.$Rid.json has no full package" }
foreach ($e in $entries) {
    if ($e.Version -ne $Version) { throw "releases.$Rid.json lists $($e.FileName) ($($e.Version)), not $Version" }
    $p = Join-Path $Dir $e.FileName
    if (-not (Test-Path -LiteralPath $p)) { throw "releases.$Rid.json lists $($e.FileName), which is not in the output" }
    if ((Get-Item -LiteralPath $p).Length -ne $e.Size) { throw "$($e.FileName): size differs from the feed" }
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $p).Hash -ne $e.SHA256) { throw "$($e.FileName): SHA256 differs from the feed" }
}

# 2. Portable.zip: launcher, updater and the app with its licence files.
$zip = [IO.Compression.ZipFile]::OpenRead((Resolve-Path $portable).Path)
try {
    $names = @($zip.Entries | ForEach-Object FullName)
    $required = '.portable', 'PR Monitor.exe', 'Update.exe', 'current/pr-monitor.exe', 'current/sq.version', 'current/LICENSE-MIT', 'current/LICENSE-APACHE'
    if (-not $NoNotices) { $required += 'current/THIRD-PARTY-LICENSES.html' }
    foreach ($want in $required) {
        if ($want -notin $names) { throw "Portable.zip lacks $want" }
    }
    # 3. The packed exe is the one verify-exe.ps1 checked.
    $stream = $zip.GetEntry('current/pr-monitor.exe').Open()
    try { $inner = (Get-FileHash -Algorithm SHA256 -InputStream $stream).Hash } finally { $stream.Dispose() }
    if ($inner -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $Exe).Hash) { throw 'current/pr-monitor.exe in Portable.zip differs from the verified exe' }
} finally { $zip.Dispose() }

$sizes = Get-ChildItem -LiteralPath $Dir -File | Where-Object Extension -in '.exe', '.zip', '.nupkg' |
    ForEach-Object { '{0} {1:N2} MiB' -f ($_.Name -replace "^$([regex]::Escape($PackId))-", ''), ($_.Length / 1MB) }
Write-Summary "package ($Rid): ok. $($sizes -join '; ')"
