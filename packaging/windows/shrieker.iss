#ifndef AppVersion
#define AppVersion "0.6.1"
#endif

#ifndef AppArch
#define AppArch "x64"
#endif

#ifndef SourceDir
#define SourceDir "."
#endif

#ifndef OutputDir
#define OutputDir "."
#endif

#ifndef IconFile
#define IconFile ".\icon.ico"
#endif

#ifndef LicenseFile
#define LicenseFile ".\LICENSE"
#endif

#if AppArch == "arm64"
#define AppSuffix "windows-arm64"
#define AllowedArchitectures "arm64"
#else
#define AppSuffix "windows-amd64"
#define AllowedArchitectures "x64compatible and not arm64"
#endif

[Setup]
AppId=io.github.kercyding.shrieker
AppName=Shrieker
AppVersion={#AppVersion}
AppPublisher=KercyDing
AppPublisherURL=https://github.com/KercyDing/shrieker
AppSupportURL=https://github.com/KercyDing/shrieker/issues
DefaultDirName={autopf}\Shrieker
DefaultGroupName=Shrieker
DisableProgramGroupPage=yes
LicenseFile={#LicenseFile}
OutputDir={#OutputDir}
OutputBaseFilename=shrieker-{#AppVersion}-{#AppSuffix}-setup
ArchitecturesAllowed={#AllowedArchitectures}
ArchitecturesInstallIn64BitMode={#AllowedArchitectures}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile={#IconFile}
UninstallDisplayIcon={app}\icon.ico

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\shrieker.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Shrieker"; Filename: "{app}\shrieker.exe"; IconFilename: "{app}\icon.ico"
Name: "{autodesktop}\Shrieker"; Filename: "{app}\shrieker.exe"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\shrieker.exe"; Description: "{cm:LaunchProgram,Shrieker}"; Flags: nowait postinstall skipifsilent
