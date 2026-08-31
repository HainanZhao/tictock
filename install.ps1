# Installs the latest `clock` release for Windows (x86_64).
#   irm https://raw.githubusercontent.com/HainanZhao/tictock/main/install.ps1 | iex
$ErrorActionPreference = "Stop"

$repo = "HainanZhao/tictock"
$installDir = "$env:LOCALAPPDATA\clock"
New-Item -ItemType Directory -Force -Path $installDir | Out-Null

$release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest"
$tag = $release.tag_name
$url = "https://github.com/$repo/releases/download/$tag/clock-x86_64-pc-windows-msvc.zip"

$zipPath = Join-Path $env:TEMP "clock.zip"
Write-Host "Downloading clock $tag for x86_64-pc-windows-msvc..."
Invoke-WebRequest -Uri $url -OutFile $zipPath
Expand-Archive -Path $zipPath -DestinationPath $installDir -Force
Remove-Item $zipPath

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$installDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$installDir", "User")
    Write-Host "Added $installDir to your user PATH. Restart your terminal to pick it up."
}

Write-Host "Installed to $installDir\clock.exe"
Write-Host "Run 'clock' to start, or 'clock --help' for options."
