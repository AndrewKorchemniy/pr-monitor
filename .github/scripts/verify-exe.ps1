#Requires -Version 7.4
<#
.SYNOPSIS
  Release-exe gate: PE machine + GUI subsystem, size budget, static CRT, ProductVersion, headless --self-test.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Exe,
    [Parameter(Mandatory)][ValidateSet('x64', 'arm64')][string] $Arch,
    [string] $ExpectedVersion = '',
    # Budget (spec §5.2). After v0.1.0: round_up(measured * 1.10, 0.5 MiB).
    [long] $MaxBytes = 14MB,
    [string] $SizeJson = '',
    [int] $SelfTestTimeoutSec = 60
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Summary([string] $Line) {
    Write-Host $Line
    if ($env:GITHUB_STEP_SUMMARY) { Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Value $Line }
}

$file = Get-Item -LiteralPath $Exe

# 1. PE header: machine + subsystem (2 = WINDOWS_GUI; release builds set windows_subsystem)
$bytes = [IO.File]::ReadAllBytes($file.FullName)
$pe = [BitConverter]::ToInt32($bytes, 0x3C)
if ([Text.Encoding]::ASCII.GetString($bytes, $pe, 4) -ne "PE`0`0") { throw "$($file.Name): not a PE image" }
$machine = [BitConverter]::ToUInt16($bytes, $pe + 4)
$wantMachine = @{ x64 = 0x8664; arm64 = 0xAA64 }[$Arch]
if ($machine -ne $wantMachine) { throw ('machine 0x{0:X4} != 0x{1:X4} ({2})' -f $machine, $wantMachine, $Arch) }
$subsystem = [BitConverter]::ToUInt16($bytes, $pe + 24 + 68)
if ($subsystem -ne 2) { throw "subsystem $subsystem != 2 (WINDOWS_GUI)" }

# 2. Size budget
if ($SizeJson) { @{ bytes = $file.Length } | ConvertTo-Json -Compress | Set-Content -LiteralPath $SizeJson }
Write-Summary ('pr-monitor.exe ({0}): {1:N0} bytes = {2:N2} MiB (budget {3:N2} MiB)' -f $Arch, $file.Length, ($file.Length / 1MB), ($MaxBytes / 1MB))
if ($file.Length -gt $MaxBytes) { throw 'binary exceeds size budget' }

# 3. No dynamic CRT (+crt-static must be in effect)
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$dumpbin = & $vswhere -latest -products * -find 'VC\Tools\MSVC\**\bin\Host*\*\dumpbin.exe' | Select-Object -First 1
if (-not $dumpbin) { throw 'dumpbin.exe not found via vswhere (install VS Build Tools: VCTools workload)' }
$imports = @(& $dumpbin /nologo /dependents $file.FullName | ForEach-Object Trim | Where-Object { $_ -match '\.dll$' })
Write-Host "imports: $($imports -join ', ')"
$crt = @($imports | Where-Object { $_ -match '^(vcruntime\d+(_\d+)?|msvcp\d+(_\w+)?|ucrtbased?|api-ms-win-crt-.+)\.dll$' })
if ($crt.Count -gt 0) { throw "dynamic CRT imports present: $($crt -join ', ')" }

# 4. Version resource (winresource: ProductVersion string = CARGO_PKG_VERSION)
if ($ExpectedVersion) {
    $pv = $file.VersionInfo.ProductVersion
    if ($pv -ne $ExpectedVersion) { throw "ProductVersion '$pv' != '$ExpectedVersion'" }
}

# 5. Headless self-test. A GUI-subsystem exe is not awaited by `&` in pwsh, so use Process with redirection.
$tmpHome = Join-Path ([IO.Path]::GetTempPath()) ('prm-selftest-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmpHome | Out-Null
$psi = [Diagnostics.ProcessStartInfo]::new($file.FullName)
$psi.ArgumentList.Add('--self-test')
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.Environment['PRMONITOR_HOME'] = $tmpHome
$p = [Diagnostics.Process]::Start($psi)
$stdout = $p.StandardOutput.ReadToEndAsync()
$stderr = $p.StandardError.ReadToEndAsync()
if (-not $p.WaitForExit($SelfTestTimeoutSec * 1000)) { $p.Kill($true); throw "--self-test timed out after $SelfTestTimeoutSec s" }
$p.WaitForExit()
Write-Host "self-test stdout: $($stdout.Result)"
if ($stderr.Result) { Write-Host "self-test stderr: $($stderr.Result)" }
if ($p.ExitCode -ne 0) { throw "--self-test exited with $($p.ExitCode)" }
if (Get-ChildItem -LiteralPath $tmpHome -Force -Recurse) { throw '--self-test must not write to PRMONITOR_HOME' }
Write-Summary "self-test ($Arch): ok"
