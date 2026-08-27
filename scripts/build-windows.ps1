$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $projectRoot

rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc

$dist = Join-Path $projectRoot "dist"
New-Item -ItemType Directory -Force -Path $dist | Out-Null
$portable = Join-Path $dist "A-Simple-Autoclicker-Windows-x64"
New-Item -ItemType Directory -Force -Path $portable | Out-Null
Copy-Item "target\x86_64-pc-windows-msvc\release\a-simple-autoclicker.exe" $portable -Force
Copy-Item "README.md", "LICENSE" $portable -Force

$zip = Join-Path $dist "A-Simple-Autoclicker-Windows-x64.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path "$portable\*" -DestinationPath $zip

Write-Host "Portable Windows build created at $zip"
Write-Host "GitHub Actions also creates the Setup executable with Inno Setup."
