#Requires -Version 5.1
<#
.SYNOPSIS
    Installs the latest parrotui-spotify release for Windows.
.DESCRIPTION
    Downloads the latest x86_64-pc-windows-msvc release asset from GitHub
    and installs it to $env:INSTALL_DIR (default: %LOCALAPPDATA%\Programs\parrotui-spotify),
    adding that directory to the user PATH if it isn't there already.
#>

$ErrorActionPreference = "Stop"

$Repo = "roamingparrot/parrotui-spotify"
$Binary = "parrotui-spotify"
$Target = "x86_64-pc-windows-msvc"
$InstallDir = if ($env:INSTALL_DIR) { $env:INSTALL_DIR } else { "$env:LOCALAPPDATA\Programs\parrotui-spotify" }

function Get-LatestTag {
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
    return $release.tag_name
}

$latest = Get-LatestTag
$url = "https://github.com/$Repo/releases/download/$latest/$Binary-$Target.zip"

Write-Host "Installing $Binary $latest ($Target)"

$tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) "parrotui-spotify-install-$([System.Guid]::NewGuid())"
New-Item -ItemType Directory -Path $tmpDir | Out-Null

try {
    $zipPath = Join-Path $tmpDir "archive.zip"
    Write-Host "Downloading $url"
    Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing

    Expand-Archive -Path $zipPath -DestinationPath $tmpDir -Force

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Move-Item -Path (Join-Path $tmpDir "$Binary.exe") -Destination (Join-Path $InstallDir "$Binary.exe") -Force

    Write-Host "Installed $Binary to $InstallDir\$Binary.exe"

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (";$userPath;" -notlike "*;$InstallDir;*") {
        [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
        $env:Path = "$env:Path;$InstallDir"
        Write-Host "Added $InstallDir to your PATH. Restart your terminal for it to take effect in new windows."
    }
}
finally {
    Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
}
