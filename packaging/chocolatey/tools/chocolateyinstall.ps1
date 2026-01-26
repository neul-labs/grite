$ErrorActionPreference = 'Stop'

$packageName = 'grit'
$version = '0.1.0'
$checksum = 'PLACEHOLDER_SHA256'
$checksumType = 'sha256'

$url64 = "https://github.com/neul-labs/grit/releases/download/v$version/grit-$version-x86_64-pc-windows-msvc.zip"

$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"
$installDir = Join-Path $toolsDir $packageName

# Download and extract
$packageArgs = @{
  packageName    = $packageName
  unzipLocation  = $installDir
  url64bit       = $url64
  checksum64     = $checksum
  checksumType64 = $checksumType
}

Install-ChocolateyZipPackage @packageArgs

# Find the extracted directory and move binaries
$extractedDir = Get-ChildItem -Path $installDir -Directory | Where-Object { $_.Name -like "grit-*" } | Select-Object -First 1
if ($extractedDir) {
  $binDir = Join-Path $installDir 'bin'
  New-Item -ItemType Directory -Force -Path $binDir | Out-Null

  Move-Item -Path (Join-Path $extractedDir.FullName 'grit.exe') -Destination (Join-Path $binDir 'grit.exe') -Force
  Move-Item -Path (Join-Path $extractedDir.FullName 'grited.exe') -Destination (Join-Path $binDir 'grited.exe') -Force

  Remove-Item -Path $extractedDir.FullName -Recurse -Force
}

# Add to PATH
$binPath = Join-Path $installDir 'bin'
Install-ChocolateyPath -PathToInstall $binPath -PathType 'Machine'

Write-Host "grit has been installed to $binPath"
Write-Host "You may need to restart your terminal for PATH changes to take effect."
