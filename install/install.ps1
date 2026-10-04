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
    irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/main/install/install.ps1 | iex

.EXAMPLE
    .\install.ps1 -Prefix C:\Tools\niki
#>

[CmdletBinding()]
param(
    [string] $Prefix = "$env:LOCALAPPDATA\niki"
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Write-Step { param([string] $Message) Write-Host "  $Message" -ForegroundColor Cyan }
function Fail { param([string] $Message) throw "error: $Message" }

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
    $dir = Split-Path -Parent $MyInvocation.MyCommand.Path
    $local = Get-ChildItem -Path $dir -Filter 'niki-*.whl' -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($local) { return $local.FullName }

    Write-Step 'no wheel alongside the installer; downloading the latest release'
    $release = Invoke-RestMethod -Uri 'https://api.github.com/repos/RavaniRoshan/niki-agent/releases/latest'
    $asset = $release.assets |
        Where-Object { $_.name -like '*windows*' } |
        Select-Object -First 1
    if (-not $asset) { Fail 'could not find a Windows release asset' }

    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    $zip = Join-Path $tmp $asset.name
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zip
    Expand-Archive -Path $zip -DestinationPath $tmp
    $wheel = Get-ChildItem -Path $tmp -Recurse -Filter 'niki-*.whl' |
        Select-Object -First 1
    if (-not $wheel) { Fail 'release asset contained no wheel' }
    return $wheel.FullName
}

function Install-Niki {
    $python = Get-Python

    & $python -c 'import sys; v=sys.version_info; print("%d.%d"%v[:2])' | ForEach-Object { $pyver = $_ }
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