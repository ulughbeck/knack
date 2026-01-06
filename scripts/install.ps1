param(
  [string]$Version
)

$Repo = "ulughbeck/knack"
$Bin = "knack.exe"

function Usage {
  @"
Usage: install.ps1 [-Version v0.0.1]

Installs the latest or specified version of $Bin from GitHub Releases.

Parameters:
  -Version   Git tag like v0.0.1 (optional)

Environment:
  INSTALL_DIR   Install destination (default: %LOCALAPPDATA%\Programs\knack\bin if writable, else %USERPROFILE%\.local\bin)
  VERSION       Same as -Version
"@ | Write-Host
}

if ($args -contains "-h" -or $args -contains "--help") {
  Usage
  exit 0
}

if (-not $Version) {
  $Version = $env:VERSION
}

if (-not $Version) {
  try {
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $release.tag_name
  } catch {
    Write-Error "Could not determine latest version."
    exit 1
  }
}

if (-not $Version) {
  Write-Error "Could not determine version to install."
  exit 1
}

$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -ne "AMD64") {
  Write-Error "Unsupported architecture: $arch"
  exit 1
}

$asset = "knack-windows-x86_64.zip"
$uri = "https://github.com/$Repo/releases/download/$Version/$asset"

$tmpRoot = Join-Path $env:TEMP "knack-install"
$rand = [System.IO.Path]::GetRandomFileName()
$tmpDir = Join-Path $tmpRoot $rand
$null = New-Item -ItemType Directory -Force -Path $tmpDir

$zipPath = Join-Path $tmpDir $asset

try {
  Invoke-WebRequest -Uri $uri -OutFile $zipPath -UseBasicParsing
} catch {
  Write-Error "Failed to download $uri"
  exit 1
}

try {
  Expand-Archive -Path $zipPath -DestinationPath $tmpDir -Force
} catch {
  Write-Error "Failed to extract $asset"
  exit 1
}

$installDir = $env:INSTALL_DIR
if (-not $installDir) {
  $preferred = Join-Path $env:LOCALAPPDATA "Programs\knack\bin"
  try {
    $null = New-Item -ItemType Directory -Force -Path $preferred
    $installDir = $preferred
  } catch {
    $fallback = Join-Path $env:USERPROFILE ".local\bin"
    $null = New-Item -ItemType Directory -Force -Path $fallback
    $installDir = $fallback
  }
}

try {
  Copy-Item -Path (Join-Path $tmpDir $Bin) -Destination (Join-Path $installDir $Bin) -Force
} catch {
  Write-Error "Failed to install to $installDir"
  exit 1
}

$pathEntries = $env:Path -split ';'
$onPath = $false
foreach ($entry in $pathEntries) {
  if ($entry.TrimEnd('\\') -ieq $installDir.TrimEnd('\\')) {
    $onPath = $true
    break
  }
}

if (-not $onPath) {
  Write-Host "Added $Bin to $installDir, but that path is not on your PATH."
  Write-Host "Add for current session:"
  Write-Host "  `$env:Path += ';$installDir'"
  Write-Host "Add permanently:"
  Write-Host "  [Environment]::SetEnvironmentVariable('Path', `$env:Path + ';$installDir', 'User')"
}

Write-Host "Installed $Bin $Version to $installDir\\$Bin"
