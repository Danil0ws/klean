$ErrorActionPreference = 'Stop'

$packageName = 'klean'
$url = 'https://github.com/danil0ws/klean/releases/download/v1.0.0/klean-v1.0.0-x86_64-pc-windows-msvc.zip'
$checksum = 'abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234'
$checksumType = 'sha256'
$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

# Download the package
$zipFile = Join-Path $env:TEMP "klean.zip"

Get-ChocolateyWebFile -PackageName $packageName `
                      -FileFullPath $zipFile `
                      -Url $url `
                      -Checksum $checksum `
                      -ChecksumType $checksumType

# Extract
Get-ChocolateyUnzip -FileFullPath $zipFile -Destination $toolsDir

# Cleanup
Remove-Item $zipFile -Force

# The binary is now available at $toolsDir\klean.exe
Write-Host "klean has been installed to $toolsDir"
