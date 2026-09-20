$ErrorActionPreference = 'Stop'

# __URL__ and __SHA__ are replaced by .github/workflows/windows-packages.yml at
# release time. Install-ChocolateyZipPackage also creates the shim, so the
# binary is on PATH as `klean` right after install.
$url = '__URL__'
$checksum = '__SHA__'

$toolsDir = Split-Path -Parent $MyInvocation.MyCommand.Definition

Install-ChocolateyZipPackage `
  -PackageName 'klean' `
  -Url $url `
  -UnzipLocation $toolsDir `
  -Checksum $checksum `
  -ChecksumType 'sha256'
