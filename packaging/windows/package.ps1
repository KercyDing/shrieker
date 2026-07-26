param(
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [Parameter(Mandatory = $true)]
    [ValidateSet("x64", "arm64")]
    [string]$Arch
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Root = Resolve-Path (Join-Path $ScriptDir "..\..")
$Binary = Join-Path $Root "target\release\shrieker.exe"
if (-not (Test-Path $Binary)) {
    throw "Missing release binary: $Binary"
}

$PackageDir = Join-Path $Root "dist\package\windows-$Arch"
$InstallerDir = Join-Path $Root "dist\installers"
New-Item -ItemType Directory -Force -Path $PackageDir, $InstallerDir | Out-Null

Copy-Item $Binary (Join-Path $PackageDir "shrieker.exe") -Force
Copy-Item (Join-Path $Root "assets\icon.ico") (Join-Path $PackageDir "icon.ico") -Force

$IsccCommand = Get-Command "ISCC.exe" -ErrorAction SilentlyContinue
if ($null -ne $IsccCommand) {
    $IsccPath = $IsccCommand.Path
} else {
    $IsccPath = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
}
if (-not (Test-Path $IsccPath)) {
    throw "Inno Setup ISCC.exe is required"
}

$NormalizedVersion = $Version.TrimStart("v")
$IssPath = Join-Path $ScriptDir "shrieker.iss"
& $IsccPath `
    "/DAppVersion=$NormalizedVersion" `
    "/DAppArch=$Arch" `
    "/DSourceDir=$PackageDir" `
    "/DOutputDir=$InstallerDir" `
    "/DIconFile=$(Join-Path $PackageDir "icon.ico")" `
    "/DLicenseFile=$(Join-Path $Root "LICENSE")" `
    $IssPath
if ($LASTEXITCODE -ne 0) {
    throw "Inno Setup failed with exit code $LASTEXITCODE"
}
