[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$installDir = $PSScriptRoot
$startMenuShortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'rustcraft.lnk'
$desktopShortcut = Join-Path ([Environment]::GetFolderPath('Desktop')) 'rustcraft.lnk'

if ($env:RUSTCRAFT_SKIP_SHORTCUTS -ne '1') {
    Remove-Item -LiteralPath $startMenuShortcut -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $desktopShortcut -Force -ErrorAction SilentlyContinue
}
if ($env:RUSTCRAFT_SKIP_REGISTRATION -ne '1') {
    Remove-Item -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\rustcraft' `
        -Recurse -Force -ErrorAction SilentlyContinue
}

try {
    Remove-Item -LiteralPath $installDir -Recurse -Force
} catch {
    throw 'Could not remove rustcraft. Close the game and run the uninstaller again.'
}

Write-Host 'rustcraft uninstalled'
