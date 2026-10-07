$ErrorActionPreference = 'Stop';

$packageName = 'gset'
$url64 = 'https://github.com/Crazygiscool/GSETLang/releases/download/v3.2.1/gset-windows-amd64.zip'
$checksum64 = 'FILL_AFTER_RELEASE'

$packageArgs = @{
  packageName   = $packageName
  unzipLocation = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"
  url64bit      = $url64
  checksum64    = $checksum64
  checksumType64= 'sha256'
}

Install-ChocolateyZipPackage @packageArgs

$exePath = Join-Path "$(Split-Path -parent $MyInvocation.MyCommand.Definition)" 'gset.exe'
if (-not (Test-Path $exePath)) {
  Write-Warning "gset.exe not found after extraction"
}
