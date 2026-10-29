; T08 candidate: compile only after staging the complete, licensed Windows payload.
#ifndef StageDir
  #error StageDir must be the absolute path to the staged payload
#endif
#ifndef OutputDir
  #error OutputDir must be the absolute path to the output directory
#endif
#if !FileExists(StageDir + "\osu-radio-qt.exe") || !FileExists(StageDir + "\osu-radio-server.exe") || !FileExists(StageDir + "\helper\osu-lazer-realm-parser.exe") || !FileExists(StageDir + "\helper\coreclr.dll") || !FileExists(StageDir + "\helper\realm-wrappers.dll") || !FileExists(StageDir + "\platforms\qwindows.dll") || !FileExists(StageDir + "\Qt6Core.dll")
  #error Staged Windows payload is missing required application or runtime files
#endif

[Setup]
AppId=osu-radio-desktop
AppName=osu! radio
AppVersion=0.1.0-alpha
DefaultDirName={autopf}\osu-radio
DefaultGroupName=osu! radio
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
SetupIconFile=..\icons\osu-radio.ico
UninstallDisplayIcon={app}\osu-radio-qt.exe
OutputDir={#OutputDir}
OutputBaseFilename=osu-radio-0.1.0-alpha-setup
Compression=lzma2
SolidCompression=yes

[Files]
Source: "{#StageDir}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\osu! radio"; Filename: "{app}\osu-radio-qt.exe"; WorkingDir: "{app}"
