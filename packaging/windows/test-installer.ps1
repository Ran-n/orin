[CmdletBinding()]
param([Parameter(Mandatory)][string]$Installer)

$ErrorActionPreference = 'Stop'
$Installer = (Resolve-Path -LiteralPath $Installer).Path
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$registryPath = 'HKCU:/Software/Microsoft/Windows/CurrentVersion/Uninstall/{A79D4647-CB82-4B48-90B2-18D32F8A43C5}_is1'
$startShortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Orin.lnk'
$desktopShortcut = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'Orin.lnk'
foreach ($existing in @($registryPath, $startShortcut, $desktopShortcut)) {
    if (Test-Path -LiteralPath $existing) { throw "An existing Orin installation or shortcut prevents this test: $existing" }
}
$targetDirectory = (Resolve-Path (Join-Path $projectRoot 'target')).Path
$scratch = Join-Path $targetDirectory ('installer-smoke-' + [guid]::NewGuid().ToString('N'))
$appDirectory = Join-Path $scratch 'app'
New-Item -ItemType Directory -Path $scratch | Out-Null

function Invoke-SetupProcess([string]$Program, [string[]]$Options) {
    $process = Start-Process -FilePath $Program -ArgumentList $Options -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "$Program failed with exit code $($process.ExitCode)." }
}

try {
    $arguments = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/TASKS=desktopicon', ('/DIR="' + $appDirectory + '"'))
    Invoke-SetupProcess $Installer $arguments
    $app = Join-Path $appDirectory 'orin.exe'
    if (!(Test-Path -LiteralPath $app)) { throw 'The installed executable is missing.' }
    $registration = Get-ItemProperty -LiteralPath $registryPath
    if ($registration.DisplayName -notlike 'Orin*') { throw 'Uninstall registration is incorrect.' }
    $shell = New-Object -ComObject WScript.Shell
    foreach ($shortcut in @($startShortcut, $desktopShortcut)) {
        if (!(Test-Path -LiteralPath $shortcut)) { throw "Shortcut is missing: $shortcut" }
        if ($shell.CreateShortcut($shortcut).TargetPath -ne $app) { throw "Shortcut target is incorrect: $shortcut" }
    }
    $output = Join-Path $scratch 'version.txt'
    $process = Start-Process -FilePath $app -ArgumentList '--version' -RedirectStandardOutput $output -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw 'The installed executable failed.' }
    if ((Get-Content -LiteralPath $output -Raw).Trim() -ne "Orin $($registration.DisplayVersion)") {
        throw 'The installer and installed executable versions disagree.'
    }
    Invoke-SetupProcess $Installer $arguments
    Write-Output 'PASS: install, both shortcuts, uninstall registration, installed version, reinstall'
}
finally {
    $uninstaller = Join-Path $appDirectory 'unins000.exe'
    if (Test-Path -LiteralPath $uninstaller) {
        Invoke-SetupProcess $uninstaller @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART')
    }
    foreach ($remaining in @($registryPath, $startShortcut, $desktopShortcut, $appDirectory)) {
        if (Test-Path -LiteralPath $remaining) { throw "Uninstall left a resource behind: $remaining" }
    }
    $resolvedScratch = (Resolve-Path -LiteralPath $scratch).Path
    if ([IO.Path]::GetDirectoryName($resolvedScratch) -ne $targetDirectory) {
        throw "Refusing to remove a scratch directory outside this project's target directory: $resolvedScratch"
    }
    Remove-Item -LiteralPath $resolvedScratch -Recurse -Force
}
Write-Output 'PASS: uninstall removed files, both shortcuts, and registration'
