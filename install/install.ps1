<#
.SYNOPSIS
    Niki Agent installer for Windows.

.DESCRIPTION
    Installs a prebuilt, self-contained `niki.exe`. There is no language runtime
    and no package resolution at install time: the release archive holds one
    executable, and the release's SHA256SUMS proves it before anything is
    written to the prefix.

.PARAMETER Prefix
    Install location. Defaults to $env:LOCALAPPDATA\niki.

.PARAMETER Version
    Release to install, without the leading v. Defaults to the latest release.

.PARAMETER Archive
    Install from this local zip instead of downloading a release.

.EXAMPLE
    irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex

.EXAMPLE
    .\install.ps1 -Prefix C:\Tools\niki
#>

[CmdletBinding()]
param(
    [string] $Prefix = "$env:LOCALAPPDATA\niki",
    [string] $Version = '',
    [string] $Archive = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# `$MyInvocation.MyCommand.Path` has no `Path` property inside a function under
# StrictMode, so resolve the script's directory once at script scope. When the
# script arrives through `iex` (the documented one-liner) this is empty and the
# install falls back to the release download.
$ScriptDir = if ($PSCommandPath) { Split-Path -Parent $PSCommandPath } else { '' }
$Repo = 'RavaniRoshan/niki-agent'
$BinDir = Join-Path $Prefix 'bin'

function Write-Step { param([string] $Message) Write-Host "  $Message" -ForegroundColor Cyan }
function Fail { param([string] $Message) throw "error: $Message" }

function Assert-Checksum {
    <# Verify a file against the release's SHA256SUMS before it is used. #>
    param([string] $File, [string] $SumsPath, [string] $Name)
    $expected = $null
    foreach ($line in (Get-Content -Path $SumsPath -ErrorAction SilentlyContinue)) {
        $parts = $line -split '\s+', 2
        if ($parts.Count -eq 2 -and $parts[1].Trim().TrimStart('*') -eq $Name) {
            $expected = $parts[0].Trim().ToLowerInvariant()
            break
        }
    }
    if (-not $expected) { Fail "SHA256SUMS has no entry for $Name" }
    $actual = (Get-FileHash -Path $File -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        Fail "checksum mismatch for $Name (expected $expected, got $actual)"
    }
    Write-Step "verified $Name"
}

function Find-Archive {
    <# Prefer a zip beside this script; fall back to the named release. #>
    if ($Archive) {
        if (-not (Test-Path $Archive)) { Fail "archive does not exist: $Archive" }
        return (Resolve-Path $Archive).Path
    }

    if ($ScriptDir) {
        # An extracted release archive holds this installer beside the binary;
        # that binary is already the one the archive shipped.
        $sibling = Join-Path $ScriptDir 'niki.exe'
        if (Test-Path $sibling) { return $sibling }
    }

    if ($ScriptDir) {
        $local = Get-ChildItem -Path $ScriptDir -Filter 'niki-*-windows-x86_64.zip' -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if ($local) {
            $localSums = Join-Path $ScriptDir 'SHA256SUMS'
            if (Test-Path $localSums) { Assert-Checksum $local.FullName $localSums $local.Name }
            return $local.FullName
        }
    }

    Write-Step 'no archive alongside the installer; downloading the release'
    $uri = if ($Version) {
        "https://api.github.com/repos/$Repo/releases/tags/v$($Version.TrimStart('v'))"
    } else {
        "https://api.github.com/repos/$Repo/releases/latest"
    }
    $release = Invoke-RestMethod -Uri $uri
    $asset = $release.assets |
        Where-Object { $_.name -like 'niki-*-windows-x86_64.zip' } |
        Select-Object -First 1
    if (-not $asset) { Fail 'this release publishes no windows-x86_64 artifact' }

    $sums = $release.assets |
        Where-Object { $_.name -eq 'SHA256SUMS' } |
        Select-Object -First 1
    if (-not $sums) { Fail 'this release publishes no SHA256SUMS; refusing to install an unverified archive' }

    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("niki-install-" + [guid]::NewGuid().ToString('n'))
    New-Item -ItemType Directory -Path $tmp | Out-Null
    $sumsPath = Join-Path $tmp 'SHA256SUMS'
    $zipPath = Join-Path $tmp $asset.name
    Invoke-WebRequest -Uri $sums.browser_download_url -OutFile $sumsPath
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zipPath
    Assert-Checksum $zipPath $sumsPath $asset.name
    return $zipPath
}

function Install-Niki {
    $archive = Find-Archive
    Write-Step "installing $(Split-Path -Leaf $archive)"

    # An extracted release archive holds the installer beside `niki.exe`; that
    # file is already the binary the archive shipped, not a zip to unpack.
    if ($archive -match '\.exe$') {
        New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
        Copy-Item -Path $archive -Destination (Join-Path $BinDir 'niki.exe') -Force
    }
    else {
        $stage = Join-Path ([System.IO.Path]::GetTempPath()) ("niki-stage-" + [guid]::NewGuid().ToString('n'))
        Expand-Archive -Path $archive -DestinationPath $stage -Force
        $binary = Get-ChildItem -Path $stage -Filter 'niki.exe' -Recurse |
            Select-Object -First 1
        if (-not $binary) { Fail 'archive contained no niki.exe' }

        New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
        Copy-Item -Path $binary.FullName -Destination (Join-Path $BinDir 'niki.exe') -Force
        Remove-Item -Path $stage -Recurse -Force -ErrorAction SilentlyContinue
    }

    & (Join-Path $BinDir 'niki.exe') --version
    Write-Host ''
    Write-Step "installed to $Prefix"
    Write-Step "add to PATH: $BinDir"
}

Install-Niki
