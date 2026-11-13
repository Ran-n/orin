[CmdletBinding()]
param(
    [string]$Binary,
    [string]$OutputDirectory,
    [string]$Compiler
)

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (!$Binary) { $Binary = Join-Path $projectRoot 'target/release/orin.exe' }
if (!$OutputDirectory) { $OutputDirectory = Join-Path $projectRoot 'dist' }
$Binary = (Resolve-Path -LiteralPath $Binary).Path
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)

if (!$Compiler) {
    $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($command) { $Compiler = $command.Source }
    else {
        $locations = @(
            "${env:ProgramFiles(x86)}/Inno Setup 6/ISCC.exe",
            "$env:ProgramFiles/Inno Setup 6/ISCC.exe",
            "$env:LOCALAPPDATA/Programs/Inno Setup 6/ISCC.exe"
        )
        $Compiler = $locations | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    }
}
if (!$Compiler) { throw 'Install Inno Setup 6 or pass -Compiler with the path to ISCC.exe.' }

Push-Location $projectRoot
try {
    $manifest = cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Could not read the Cargo package version.' }
    $version = ($manifest.packages | Where-Object name -EQ 'orin').version
    if (!$version) { throw 'The orin package version is missing.' }
    & $Compiler "/DAppVersion=$version" "/DAppBinary=$Binary" "/DDistDir=$OutputDirectory" (Join-Path $PSScriptRoot 'orin.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed.' }
    $installer = Join-Path $OutputDirectory "orin-$version-windows-x86_64-setup.exe"
    if (!(Test-Path -LiteralPath $installer)) { throw 'The installer was not produced.' }
    Write-Output $installer
}
finally { Pop-Location }
