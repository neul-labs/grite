$ErrorActionPreference = 'Stop'

$packageName = 'grit'
$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"
$installDir = Join-Path $toolsDir $packageName

# Remove from PATH
$binPath = Join-Path $installDir 'bin'
$machinePath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
$newPath = ($machinePath -split ';' | Where-Object { $_ -ne $binPath }) -join ';'
[Environment]::SetEnvironmentVariable('Path', $newPath, 'Machine')

# Remove installation directory
if (Test-Path $installDir) {
  Remove-Item -Path $installDir -Recurse -Force
}

Write-Host "grit has been uninstalled."
