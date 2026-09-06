# rusthrc installer for Windows.
#
#   irm https://raw.githubusercontent.com/prototypeall850-creator/rusthrc/main/install.ps1 | iex
#
# Pin a version:  $env:RUSTHRC_VERSION = "v0.1.0"; irm ... | iex
param(
    [string]$Version = $env:RUSTHRC_VERSION,
    [string]$InstallDir = $env:RUSTHRC_INSTALL_DIR
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$Repo = "prototypeall850-creator/rusthrc"
$Target = "x86_64-pc-windows-msvc"

function Write-Step($msg) { Write-Host "`u{00b7} $msg" -ForegroundColor Cyan }
function Write-Fail($msg) { Write-Host "`u{2717} error: $msg" -ForegroundColor Red; exit 1 }

if (-not [Environment]::Is64BitOperatingSystem) {
    Write-Fail "only 64-bit Windows is supported (x86_64-pc-windows-msvc)"
}
if (-not $Version) { $Version = "latest" }
if (-not $InstallDir) { $InstallDir = Join-Path $env:LOCALAPPDATA "Programs\rusthrc" }

if ($Version -eq "latest") {
    Write-Step "resolving the latest release..."
    $tag = (Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest").tag_name
} elseif ($Version -like "v*") {
    $tag = $Version
} else {
    $tag = "v$Version"
}
$ver = $tag.TrimStart("v")
$asset = "rusthrc-$ver-$Target.zip"
$url = "https://github.com/$Repo/releases/download/$tag/$asset"

$tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    Write-Step "downloading $asset..."
    Invoke-WebRequest -Uri $url -OutFile (Join-Path $tmp $asset)
    $zip = Join-Path $tmp $asset

    try {
        Invoke-WebRequest -Uri "$url.sha256" -OutFile "$zip.sha256"
        $expected = ((Get-Content "$zip.sha256" -Raw) -split '\s+')[0].Trim().ToLower()
        $actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
        if ($expected -ne $actual) { Write-Fail "checksum mismatch for $asset" }
        Write-Step "checksum verified"
    } catch {
        Write-Step "checksum file unavailable — skipping verification"
    }

    Expand-Archive -Path $zip -DestinationPath $tmp -Force
    $exe = Join-Path $tmp "rusthrc-$ver-$Target\rusthrc.exe"
    if (-not (Test-Path $exe)) { Write-Fail "unexpected archive layout" }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item $exe (Join-Path $InstallDir "rusthrc.exe") -Force
    Write-Step "installed $InstallDir\rusthrc.exe"

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (($userPath -split ";") -notcontains $InstallDir) {
        [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
        Write-Step "added $InstallDir to your user PATH — restart your terminal to pick it up"
    }

    & (Join-Path $InstallDir "rusthrc.exe") --version
} finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
