#Requires -Version 7.4
<#
.SYNOPSIS
  Packs the app with Velopack for one runtime: Windows gives Setup.exe, Portable.zip and the update
  packages; Linux gives a self-updating .AppImage and the update packages.
  Runs in CI (release.yml, ci.yml) and locally; needs the .NET SDK to install vpk.

.DESCRIPTION
  1. Installs the pinned vpk into a tool path (no global state).
  2. Stages the pack dir: the app exe as pr-monitor.exe plus the licence files.
  3. Downloads the previous published release, so vpk can build a delta package.
  4. Runs `vpk pack`.
  5. Copies only this build's files into -OutDir and filters releases.<rid>.json to this version.
  6. Runs verify-package.ps1 on the result.

  Output files in -OutDir (names are vpk's; <id> = pr-monitor):
    <id>-win-<arch>-Setup.exe, <id>-win-<arch>-Portable.zip   (Windows)
    <id>-linux-<arch>.AppImage                                (Linux)
    <id>-<version>-win-<arch>-full.nupkg, <id>-<version>-win-<arch>-delta.nupkg (when there is a previous release),
    releases.win-<arch>.json
  Never publishes anything: uploading to the draft release is release.yml's job.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('win-x64', 'win-arm64', 'linux-x64', 'linux-arm64')][string] $Rid,
    [Parameter(Mandatory)][string] $Exe,
    # Default (Windows only): the exe's ProductVersion (CARGO_PKG_VERSION, set by winresource). Required on Linux.
    [string] $Version = '',
    # Default: the icon build.rs generated next to the exe (target/<triple>/release/build/pr-monitor-*/out/):
    # app.ico on Windows, icon-256.png on Linux (vpk needs a PNG there).
    [string] $Icon = '',
    [string] $OutDir = 'velopack-out',
    # Folder holding THIRD-PARTY-LICENSES.html (the notices artifact); optional.
    [string] $NoticesDir = '',
    # Markdown release notes stored in the package; optional.
    [string] $NotesFile = '',
    # Dry run (CI): skip downloading the previous release (no delta package) and tolerate a missing velopack crate.
    [switch] $NoDownload,
    [string] $Repo = $(if ($env:GITHUB_REPOSITORY) { $env:GITHUB_REPOSITORY } else { 'AndrewKorchemniy/pr-monitor' }),
    # Where vpk is installed; reused when it is already there.
    [string] $ToolPath = (Join-Path ([IO.Path]::GetTempPath()) 'prm-vpk')
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/common.ps1"

# Keep equal to the `velopack` crate version in Cargo.lock (the guard below enforces it).
$VpkVersion = '1.2.161'
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')
$exeFile = Get-Item -LiteralPath $Exe
$platform = Get-Platform $Rid
$mainExe = if ($platform -eq 'windows') { 'pr-monitor.exe' } else { 'pr-monitor' }

# The crate and the CLI must agree, or vpk may write package metadata the app cannot read.
$lock = Join-Path $root 'Cargo.lock'
$crate = if (Test-Path $lock) { [regex]::Match((Get-Content -Raw $lock), '(?m)^name = "velopack"\r?\nversion = "([^"]+)"') } else { $null }
if ($crate -and $crate.Success) {
    if ($crate.Groups[1].Value -ne $VpkVersion) {
        throw "velopack crate $($crate.Groups[1].Value) in Cargo.lock != vpk $VpkVersion; update `$VpkVersion in $PSCommandPath"
    }
} elseif ($NoDownload) {
    Write-Host "::notice::no velopack crate in Cargo.lock; skipping the vpk version check in this dry run"
} else {
    # why: without the crate the app never runs VelopackApp, so the package would install but could not update.
    throw 'the velopack crate is not in Cargo.lock; refusing to pack a release the app cannot update from'
}

if (-not $Version -and $platform -eq 'windows') { $Version = $exeFile.VersionInfo.ProductVersion }
if (-not $Version) { throw "no -Version given and $($exeFile.Name) has no ProductVersion to read" }

if (-not $Icon) {
    $iconName = if ($platform -eq 'windows') { 'app.ico' } else { 'icon-256.png' }
    $found = Get-ChildItem -Path (Join-Path $exeFile.DirectoryName "build/pr-monitor-*/out/$iconName") -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending
    if (-not $found) { throw "no $iconName under $($exeFile.DirectoryName)/build; pass -Icon" }
    $Icon = $found[0].FullName
}

# 1. vpk
$vpk = Join-Path $ToolPath (@('vpk', 'vpk.exe')[[int]$IsWindows])
if (-not (Test-Path $vpk)) {
    Invoke-Native dotnet @('tool', 'install', 'vpk', '--version', $VpkVersion, '--tool-path', $ToolPath)
}

# 2. Stage the pack dir. The app exe is always called pr-monitor(.exe) inside the package.
$work = Join-Path ([IO.Path]::GetTempPath()) "prm-pack-$Rid-$([guid]::NewGuid())"
$stage = Join-Path $work 'stage'
$out = Join-Path $work 'out'
New-Item -ItemType Directory -Path (Join-Path $stage 'licenses'), $out | Out-Null
Copy-Item -LiteralPath $exeFile.FullName -Destination (Join-Path $stage $mainExe)
if ($platform -eq 'linux') {
    if ($IsWindows) { Write-Warning 'packing a Linux runtime on Windows: the binary will not be executable; use this only to check the packaging, not to ship' }
    else { Invoke-Native chmod @('+x', (Join-Path $stage $mainExe)) }
}
foreach ($f in 'LICENSE-MIT', 'LICENSE-APACHE', 'README.md') { Copy-Item (Join-Path $root $f) $stage }
Copy-Item (Join-Path $root 'crates/prm-ui/assets/fonts/*.txt') (Join-Path $stage 'licenses')
if ($NoticesDir) { Copy-Item (Join-Path $NoticesDir 'THIRD-PARTY-LICENSES.html') $stage }

# 3. Previous release, for the delta package. Having no release is not an error.
#    Anonymous on purpose: with a token the API also lists drafts, so a re-run (recover) would
#    offer this release's own draft as the "previous" one. The price is the unauthenticated limit
#    of 60 API requests per hour per IP, which GitHub-hosted runners share, so a busy IP can be
#    rate limited. Retry for about ten minutes before giving up (the job can be re-run via recover).
if (-not $NoDownload) {
    $attempt = 0
    while ($true) {
        try {
            Invoke-Native $vpk @('download', 'github', '--repoUrl', "https://github.com/$Repo", '--channel', $Rid, '--outputDir', $out)
            break
        } catch {
            if (++$attempt -ge 5) { throw }
            Write-Host "::warning::vpk download failed ($($_.Exception.Message)); retrying"
            Start-Sleep -Seconds (60 * $attempt)
        }
    }
}

# 4. Pack.
$pack = @(
    'pack', '--packId', $PackId, '--packVersion', $Version, '--packDir', $stage,
    '--mainExe', $mainExe, '--packTitle', 'PR Monitor', '--packAuthors', 'Andrew Korchemniy',
    '--icon', $Icon, '--channel', $Rid, '--runtime', $Rid, '--outputDir', $out
)
if ($platform -eq 'windows') {
    # why: vpk's default AUMID differs, which would break toast activation from the shortcut.
    $pack += '--aumid', $Aumid, '--shortcuts', 'StartMenuRoot'
} else {
    # `[linux]` selects vpk's Linux pack command; --aumid and --shortcuts do not exist there.
    $pack = @('[linux]') + $pack + @('--categories', 'Development;')
}
if ($NotesFile) { $pack += '--releaseNotes', (Resolve-Path $NotesFile).Path }
Invoke-Native $vpk $pack

# 5. Keep this build's files only. `download` left the previous full package in $out, and the
#    pack's releases.<rid>.json lists it too. Velopack's docs want each release to carry one full
#    and one delta package, and `vpk upload` itself builds the feed from the current build's
#    assets only, so we upload exactly that.
$assets = Get-Content -Raw (Join-Path $out "assets.$Rid.json") | ConvertFrom-Json
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
foreach ($a in $assets) { Copy-Item (Join-Path $out $a.RelativeFileName) $OutDir -Force }
$feed = Get-Content -Raw (Join-Path $out "releases.$Rid.json") | ConvertFrom-Json
$feed.Assets = @($feed.Assets | Where-Object { $_.Version -eq $Version })
$feed | ConvertTo-Json -Depth 8 -Compress | Set-Content -LiteralPath (Join-Path $OutDir "releases.$Rid.json") -NoNewline

# 6. Check the result.
$verify = @{ Dir = $OutDir; Rid = $Rid; Version = $Version; Exe = $exeFile.FullName; NoNotices = -not $NoticesDir }
& "$PSScriptRoot/verify-package.ps1" @verify
Remove-Item -Recurse -Force $work
