#ifndef AppVersion
  #error AppVersion is required
#endif
#ifndef AppBinary
  #error AppBinary is required
#endif
#ifndef DistDir
  #error DistDir is required
#endif

[Setup]
AppId={{A79D4647-CB82-4B48-90B2-18D32F8A43C5}
AppName=Orin
AppVersion={#AppVersion}
AppPublisher=Breren
AppPublisherURL=https://github.com/Ran-n/orin
DefaultDirName={localappdata}\Programs\Orin
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
DisableProgramGroupPage=yes
OutputDir={#DistDir}
OutputBaseFilename=orin-{#AppVersion}-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\orin.exe
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#AppBinary}"; DestDir: "{app}"; DestName: "orin.exe"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Orin"; Filename: "{app}\orin.exe"
Name: "{userdesktop}\Orin"; Filename: "{app}\orin.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\orin.exe"; Description: "{cm:LaunchProgram,Orin}"; Flags: nowait postinstall skipifsilent
