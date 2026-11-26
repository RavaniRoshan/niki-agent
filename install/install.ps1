<#
.SYNOPSIS
    Niki Agent installer for Windows.

.DESCRIPTION
    Creates an isolated virtual environment and installs the pinned wheel that
    ships beside this script, then writes a launcher to the install prefix.
    Nothing outside the prefix is modified.

.PARAMETER Prefix
    Install location. Defaults to $env:LOCALAPPDATA\niki.

.EXAMPLE
    irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex

.EXAMPLE
    .\install.ps1 -Prefix C:\Tools\niki
#>

[CmdletBinding()]
param(
    [string] $Prefix = "$env:LOCALAPPDATA\niki"
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# `$MyInvocation.MyCommand.Path` has no `Path` property inside a function under
# StrictMode, so resolve the script's directory once at script scope. When the
# script arrives through `iex` (the documented one-liner) this is empty and the
# install falls back to the release download.
$ScriptDir = if ($PSCommandPath) { Split-Path -Parent $PSCommandPath } else { '' }

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

function Get-Python {
    <# Prefers the launcher shim, then python3, then python. #>
    foreach ($candidate in @('python3', 'python')) {
        $cmd = Get-Command $candidate -ErrorAction SilentlyContinue
        if ($cmd) { return $cmd.Source }
    }
    Fail 'python is required. Install Python 3.12+ from https://python.org'
}

function Find-Wheel {
    <# Prefer a wheel beside this script; fall back to the newest release. #>
    if ($ScriptDir) {
        $local = Get-ChildItem -Path $ScriptDir -Filter 'niki_agent-*.whl' -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if ($local) {
            $localSums = Join-Path $ScriptDir 'SHA256SUMS'
            if (Test-Path $localSums) { Assert-Checksum $local.FullName $localSums $local.Name }
            return $local.FullName
        }
    }

    Write-Step 'no wheel alongside the installer; downloading the latest release'
    $release = Invoke-RestMethod -Uri 'https://api.github.com/repos/RavaniRoshan/niki-agent/releases/latest'
    $asset = $release.assets |
        Where-Object { $_.name -like '*windows*' } |
        Select-Object -First 1
    if (-not $asset) { Fail 'could not find a Windows release asset' }
    $sumsAsset = $release.assets |
        Where-Object { $_.name -eq 'SHA256SUMS' } |
        Select-Object -First 1
    if (-not $sumsAsset) {
        Fail 'this release publishes no SHA256SUMS; refusing to install an unverified archive'
    }

    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    $zip = Join-Path $tmp $asset.name
    $sumsPath = Join-Path $tmp 'SHA256SUMS'
    Invoke-WebRequest -Uri $sumsAsset.browser_download_url -OutFile $sumsPath
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zip
    Assert-Checksum $zip $sumsPath $asset.name

    Expand-Archive -Path $zip -DestinationPath $tmp
    $wheel = Get-ChildItem -Path $tmp -Recurse -Filter 'niki_agent-*.whl' |
        Select-Object -First 1
    if (-not $wheel) { Fail 'release asset contained no wheel' }
    return $wheel.FullName
}

function Install-Niki {
    $python = Get-Python

    # Single quotes inside a double-quoted PS string: PowerShell 5.1 drops
    # embedded double quotes when it hands an argument to a native process.
    $pyver = (& $python -c "import sys; print('{}.{}'.format(*sys.version_info[:2]))").Trim()
    if ([version]$pyver -lt [version]'3.12') {
        Fail "Python 3.12+ required, found $pyver"
    }

    $wheel = Find-Wheel
    Write-Step "installing $(Split-Path -Leaf $wheel)"

    New-Item -ItemType Directory -Force -Path $Prefix | Out-Null
    & $python -m venv (Join-Path $Prefix 'venv')
    $venvPy = Join-Path $Prefix 'venv\Scripts\python.exe'
    & $venvPy -m pip install --quiet --upgrade pip
    & $venvPy -m pip install --quiet $wheel

    $bin = Join-Path $Prefix 'bin'
    New-Item -ItemType Directory -Force -Path $bin | Out-Null
    Copy-Item (Join-Path $Prefix 'venv\Scripts\niki.exe') (Join-Path $bin 'niki.exe') -Force

    # Offer to put the launcher on PATH. Only when asked, so a reinstall never
    # rewrites the user's profile.
    if ($env:NIKI_SHELL -eq '1') {
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        if ($userPath -notlike "*$bin*") {
            [Environment]::SetEnvironmentVariable('Path', "$userPath;$bin", 'User')
            Write-Step "added to your user PATH (restart your shell to pick it up)"
        }
    }

    & (Join-Path $bin 'niki.exe') --version
    Write-Host ''
    Write-Step "installed to $Prefix"
    Write-Step "add to PATH now:  `$env:Path = '$bin;' + `$env:Path"
    Write-Step 're-run with $env:NIKI_SHELL=1 to persist that'
}

Install-Niki