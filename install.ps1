# Installs the latest `tictock` release for Windows (x86_64).
#   irm https://raw.githubusercontent.com/HainanZhao/tictock/main/install.ps1 | iex
$ErrorActionPreference = "Stop"

$repo = "HainanZhao/tictock"
$installDir = "$env:LOCALAPPDATA\tictock"
New-Item -ItemType Directory -Force -Path $installDir | Out-Null

$release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest"
$tag = $release.tag_name
$url = "https://github.com/$repo/releases/download/$tag/tictock-x86_64-pc-windows-msvc.zip"

$zipPath = Join-Path $env:TEMP "tictock.zip"
Write-Host "Downloading tictock $tag for x86_64-pc-windows-msvc..."
Invoke-WebRequest -Uri $url -OutFile $zipPath
Expand-Archive -Path $zipPath -DestinationPath $installDir -Force
Remove-Item $zipPath

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$installDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$installDir", "User")
    Write-Host "Added $installDir to your user PATH. Restart your terminal to pick it up."
}

Write-Host "Installed to $installDir\tictock.exe"
Write-Host "Run 'tictock' to start, or 'tictock --help' for options."
