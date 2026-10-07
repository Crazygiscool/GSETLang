$ErrorActionPreference = 'Stop'
$packageName = 'gset'
$exePath = Join-Path "$(Split-Path -parent $MyInvocation.MyCommand.Definition)" 'gset.exe'
if (Test-Path $exePath) {
  Remove-Item $exePath -Force
}
