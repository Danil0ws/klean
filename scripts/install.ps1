# klean installer for Windows (PowerShell 5+ / pwsh).
#
#   irm https://raw.githubusercontent.com/danil0ws/klean/main/scripts/install.ps1 | iex
#   .\install.ps1 -Version v1.0.0 -Prefix C:\Tools\klean
#
# Downloads the release zip, verifies it against the release SHA256SUMS and puts
# klean.exe on the user PATH. No package manager required.
[CmdletBinding()]
param(
    [string]$Version = 'latest',
    [string]$Prefix = "$env:LOCALAPPDATA\Programs\klean",
    [string]$Repo = 'danil0ws/klean',
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'aarch64' } else { 'x86_64' }
if ($arch -ne 'x86_64') {
    Write-Warning "sem build oficial para $arch; usando x86_64 (roda emulada)"
}
$target = 'x86_64-pc-windows-msvc'

if ($Version -eq 'latest') {
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $release.tag_name
    if (-not $Version) { throw 'não consegui descobrir a última release (rede ou rate limit)' }
}

$asset = "klean-$Version-$target.zip"
$base = "https://github.com/$Repo/releases/download/$Version"
Write-Host "klean $Version para $target"
Write-Host "  de:   $base/$asset"
Write-Host "  para: $Prefix\klean.exe"

if ($DryRun) {
    Write-Host '(dry-run) nada foi baixado nem instalado'
    return
}

$tmp = Join-Path $env:TEMP ("klean-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp -Force | Out-Null

try {
    $zip = Join-Path $tmp $asset
    Invoke-WebRequest -Uri "$base/$asset" -OutFile $zip -UseBasicParsing

    try {
        $sums = (Invoke-WebRequest -Uri "$base/SHA256SUMS" -UseBasicParsing).Content
        $line = ($sums -split "`n" | Where-Object { $_ -match " $([regex]::Escape($asset))$" } | Select-Object -First 1)
        if ($line) {
            $expected = ($line -split '\s+')[0]
            $actual = (Get-FileHash -Algorithm SHA256 -Path $zip).Hash.ToLower()
            if ($expected -ne $actual) { throw "checksum não bate: esperado $expected, veio $actual" }
            Write-Host '  checksum ok'
        } else {
            Write-Warning "$asset não está no SHA256SUMS"
        }
    } catch {
        Write-Warning "pulando verificação de checksum: $($_.Exception.Message)"
    }

    Expand-Archive -Path $zip -DestinationPath $tmp -Force
    New-Item -ItemType Directory -Path $Prefix -Force | Out-Null
    Copy-Item -Path (Join-Path $tmp 'klean.exe') -Destination (Join-Path $Prefix 'klean.exe') -Force

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($userPath -notlike "*$Prefix*") {
        [Environment]::SetEnvironmentVariable('Path', "$userPath;$Prefix", 'User')
        Write-Host "  adicionei $Prefix ao PATH do usuário (reabra o terminal)"
    }

    Write-Host "✓ instalado em $Prefix\klean.exe"
    & (Join-Path $Prefix 'klean.exe') --version
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
