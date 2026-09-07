[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$appName = 'rustcraft'
$version = '0.1.0'
$defaultInstallDir = Join-Path $env:LOCALAPPDATA 'Programs\rustcraft'
$installDir = if ($env:RUSTCRAFT_INSTALL_DIR) {
    [System.IO.Path]::GetFullPath($env:RUSTCRAFT_INSTALL_DIR)
} else {
    $defaultInstallDir
}
$executable = Join-Path $installDir 'rustcraft.exe'
$uninstaller = Join-Path $installDir 'uninstall.ps1'

New-Item -ItemType Directory -Path $installDir -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'rustcraft.exe') -Destination $executable -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'uninstall.ps1') -Destination $uninstaller -Force

if ($env:RUSTCRAFT_SKIP_SHORTCUTS -ne '1') {
    $shell = New-Object -ComObject WScript.Shell
    $startMenuShortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'rustcraft.lnk'
    $desktopShortcut = Join-Path ([Environment]::GetFolderPath('Desktop')) 'rustcraft.lnk'

    foreach ($shortcutPath in @($startMenuShortcut, $desktopShortcut)) {
        $shortcut = $shell.CreateShortcut($shortcutPath)
        $shortcut.TargetPath = $executable
        $shortcut.WorkingDirectory = $installDir
        $shortcut.Description = 'rustcraft voxel sandbox'
        $shortcut.IconLocation = "$executable,0"
        $shortcut.Save()
    }
}

if ($env:RUSTCRAFT_SKIP_REGISTRATION -ne '1') {
    $uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\rustcraft'
    New-Item -Path $uninstallKey -Force | Out-Null
    $uninstallCommand = '"{0}" -NoProfile -ExecutionPolicy Bypass -File "{1}"' -f `
        (Join-Path $PSHOME 'powershell.exe'), $uninstaller
    $estimatedSize = [Math]::Ceiling((Get-Item -LiteralPath $executable).Length / 1KB)

    New-ItemProperty -Path $uninstallKey -Name DisplayName -Value $appName -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $uninstallKey -Name DisplayVersion -Value $version -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $uninstallKey -Name Publisher -Value 'rustcraft' -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $uninstallKey -Name InstallLocation -Value $installDir -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $uninstallKey -Name DisplayIcon -Value $executable -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $uninstallKey -Name UninstallString -Value $uninstallCommand -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $uninstallKey -Name EstimatedSize -Value $estimatedSize -PropertyType DWord -Force | Out-Null
    New-ItemProperty -Path $uninstallKey -Name NoModify -Value 1 -PropertyType DWord -Force | Out-Null
    New-ItemProperty -Path $uninstallKey -Name NoRepair -Value 1 -PropertyType DWord -Force | Out-Null
}

Write-Host "rustcraft $version installed to $installDir"
