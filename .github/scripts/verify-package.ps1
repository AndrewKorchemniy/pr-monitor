#Requires -Version 7.4
<#
.SYNOPSIS
  Velopack package gate: the files pack-velopack.ps1 produced are complete, consistent and carry the app exe.
  It does not run anything: --self-test already ran on the app exe (verify-exe.ps1) and pack copies it unchanged.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Dir,
    [Parameter(Mandatory)][ValidateSet('win-x64', 'win-arm64', 'linux-x64', 'linux-arm64')][string] $Rid,
    [Parameter(Mandatory)][string] $Version,
    # The app exe that was packed (Windows: compared with the exe in Portable.zip; Linux: not read, the binary sits inside the AppImage).
    [Parameter(Mandatory)][string] $Exe,
    # The package was made without THIRD-PARTY-LICENSES.html (CI dry runs do not build the notices).
    [switch] $NoNotices
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/common.ps1"
Add-Type -AssemblyName System.IO.Compression.FileSystem

$platform = Get-Platform $Rid

# SHA256 of one entry inside a zip (Portable.zip, a nupkg).
function Get-ZipEntryHash([IO.Compression.ZipArchive] $Zip, [string] $Name) {
    $entry = $Zip.GetEntry($Name)
    if (-not $entry) { throw "no $Name in the archive" }
    $stream = $entry.Open()
    try { (Get-FileHash -Algorithm SHA256 -InputStream $stream).Hash } finally { $stream.Dispose() }
}
$full = Join-Path $Dir "$PackId-$Version-$Rid-full.nupkg"
$files = if ($platform -eq 'windows') {
    $portable = Join-Path $Dir "$PackId-$Rid-Portable.zip"
    (Join-Path $Dir "$PackId-$Rid-Setup.exe"), $portable, $full
} else {
    $appImage = Join-Path $Dir "$PackId-$Rid.AppImage"
    $appImage, $full
}
foreach ($f in $files) {
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

if ($platform -eq 'windows') {
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
        $inner = Get-ZipEntryHash $zip 'current/pr-monitor.exe'
        if ($inner -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $Exe).Hash) { throw 'current/pr-monitor.exe in Portable.zip differs from the verified exe' }
    } finally { $zip.Dispose() }
} else {
    # 2. AppImage: an ELF file (the runtime + squashfs). The app binary inside it was checked by verify-elf.sh
    #    and is smoke-tested by running the AppImage (smoke-appimage.sh).
    $magic = [byte[]]::new(4)
    $fs = [IO.File]::OpenRead((Resolve-Path $appImage).Path)
    try { [void]$fs.Read($magic, 0, 4) } finally { $fs.Dispose() }
    if ([Text.Encoding]::ASCII.GetString($magic, 1, 3) -ne 'ELF' -or $magic[0] -ne 0x7F) { throw "$(Split-Path -Leaf $appImage) is not an ELF file" }
    # 3. The full package carries that same AppImage, so the update path installs what users download.
    $zip = [IO.Compression.ZipFile]::OpenRead((Resolve-Path $full).Path)
    try {
        $inner = Get-ZipEntryHash $zip "lib/app/$PackId.AppImage"
        if ($inner -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $appImage).Hash) { throw 'the AppImage in the full package differs from the downloadable AppImage' }
    } finally { $zip.Dispose() }
}

$sizes = Get-ChildItem -LiteralPath $Dir -File | Where-Object { $_.Extension -in '.exe', '.zip', '.nupkg', '.AppImage' } |
    ForEach-Object { '{0} {1:N2} MiB' -f ($_.Name -replace "^$([regex]::Escape($PackId))-", ''), ($_.Length / 1MB) }
Write-Summary "package ($Rid): ok. $($sizes -join '; ')"
