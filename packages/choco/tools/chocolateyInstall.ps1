$ErrorActionPreference = 'Stop';

$packageName = 'gset'
$url64 = 'https://github.com/Crazygiscool/GSETLang/releases/download/v3.2.1/gset-windows-amd64.zip'
$checksum64 = 'ac586e86d78f536994fddccd48a668fddc7e95f9f199f2ba58e12e6a013c8baf'

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
