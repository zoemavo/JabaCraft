[CmdletBinding()]
param(
    [string]$CargoTargetDir = '',
    [switch]$SkipCargoBuild
)

$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path $PSScriptRoot -Parent
$version = '0.1.0'
$targetDir = if ($CargoTargetDir) {
    [System.IO.Path]::GetFullPath($CargoTargetDir)
} elseif ($env:CARGO_TARGET_DIR) {
    [System.IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
} else {
    Join-Path $projectRoot 'target'
}
$releaseExe = Join-Path $targetDir 'release\rustcraft.exe'
$distDir = Join-Path $projectRoot 'dist'
$stagingDir = Join-Path $projectRoot 'installer\.staging'
$setupExe = Join-Path $distDir "rustcraft-$version-windows-x86_64-setup.exe"
$portableExe = Join-Path $distDir 'rustcraft.exe'
$sedPath = Join-Path $stagingDir 'rustcraft.sed'

if (-not $SkipCargoBuild) {
    $env:CARGO_TARGET_DIR = $targetDir
    & cargo build --release --locked
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }
}

if (-not (Test-Path -LiteralPath $releaseExe -PathType Leaf)) {
    throw "Release executable not found: $releaseExe"
}

New-Item -ItemType Directory -Path $distDir -Force | Out-Null
New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null
Copy-Item -LiteralPath $releaseExe -Destination (Join-Path $stagingDir 'rustcraft.exe') -Force
Copy-Item -LiteralPath (Join-Path $projectRoot 'installer\install.cmd') -Destination $stagingDir -Force
Copy-Item -LiteralPath (Join-Path $projectRoot 'installer\install.ps1') -Destination $stagingDir -Force
Copy-Item -LiteralPath (Join-Path $projectRoot 'installer\uninstall.ps1') -Destination $stagingDir -Force
Copy-Item -LiteralPath $releaseExe -Destination $portableExe -Force

$sourceDirectory = "$stagingDir\"
$sed = @"
[Version]
Class=IEXPRESS
SEDVersion=3

[Options]
PackagePurpose=InstallApp
ShowInstallProgramWindow=0
HideExtractAnimation=1
UseLongFileName=1
InsideCompressed=0
CAB_FixedSize=0
CAB_ResvCodeSigning=0
RebootMode=N
InstallPrompt=%InstallPrompt%
DisplayLicense=%DisplayLicense%
FinishMessage=%FinishMessage%
TargetName=%TargetName%
FriendlyName=%FriendlyName%
AppLaunched=%AppLaunched%
PostInstallCmd=%PostInstallCmd%
AdminQuietInstCmd=%AdminQuietInstCmd%
UserQuietInstCmd=%UserQuietInstCmd%
SourceFiles=SourceFiles

[Strings]
InstallPrompt=
DisplayLicense=
FinishMessage=rustcraft has been installed.
TargetName=$setupExe
FriendlyName=rustcraft Setup
AppLaunched=install.cmd
PostInstallCmd=<None>
AdminQuietInstCmd=install.cmd
UserQuietInstCmd=install.cmd
FILE0=rustcraft.exe
FILE1=install.cmd
FILE2=install.ps1
FILE3=uninstall.ps1

[SourceFiles]
SourceFiles0=$sourceDirectory

[SourceFiles0]
%FILE0%=
%FILE1%=
%FILE2%=
%FILE3%=
"@
Set-Content -LiteralPath $sedPath -Value $sed -Encoding Ascii

Remove-Item -LiteralPath $setupExe -Force -ErrorAction SilentlyContinue
$iexpress = Start-Process -FilePath "$env:WINDIR\System32\iexpress.exe" `
    -ArgumentList @('/N', '/Q', $sedPath) -WindowStyle Hidden -Wait -PassThru
if ($iexpress.ExitCode -ne 0) {
    throw "IExpress failed with exit code $($iexpress.ExitCode)"
}

# IExpress can return before its MakeCab child has finished writing the package.
$packageDeadline = [DateTime]::UtcNow.AddMinutes(2)
while (-not (Test-Path -LiteralPath $setupExe -PathType Leaf) -and
    [DateTime]::UtcNow -lt $packageDeadline) {
    Start-Sleep -Milliseconds 250
}
if (-not (Test-Path -LiteralPath $setupExe -PathType Leaf)) {
    throw "Installer was not created: $setupExe"
}

Remove-Item -LiteralPath $stagingDir -Recurse -Force

Write-Host "Portable executable: $portableExe"
Write-Host "Installer: $setupExe"
Get-FileHash -Algorithm SHA256 -LiteralPath $portableExe, $setupExe |
    Format-Table Path, Hash -AutoSize
