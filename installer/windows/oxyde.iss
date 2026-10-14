; The GPUI identity is intentionally independent of the Tauri installation.
#ifndef AppVersion
  #error AppVersion must be provided by scripts/build-distribution.ps1
#endif
#ifndef PayloadDir
  #error PayloadDir must be provided by scripts/build-distribution.ps1
#endif

[Setup]
AppId=Oxyde.GPUI.Windows
AppName=Oxyde GPUI
AppVersion={#AppVersion}
AppPublisher=Carlton2001
DefaultDirName={localappdata}\Programs\Oxyde_GPUI
DefaultGroupName=Oxyde GPUI
DisableProgramGroupPage=yes
DisableDirPage=no
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
UninstallDisplayName=Oxyde GPUI
UninstallDisplayIcon={app}\oxyde.ico
SetupIconFile={#PayloadDir}\oxyde.ico
OutputBaseFilename=Oxyde_GPUI_Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Never ask Restart Manager to close the parallel Tauri version or its workers.
CloseApplications=no
RestartApplications=no

[Languages]
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "it"; MessagesFile: "compiler:Languages\Italian.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#PayloadDir}\Oxyde_GPUI.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\oxyde-image-worker.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\oxyde-archive-io-worker.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\oxyde-archive-list-worker.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\oxyde.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\*.dll"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Oxyde GPUI"; Filename: "{app}\Oxyde_GPUI.exe"; WorkingDir: "{app}"; IconFilename: "{app}\oxyde.ico"; AppUserModelID: "Oxyde.GPUI.Windows"
Name: "{autodesktop}\Oxyde GPUI"; Filename: "{app}\Oxyde_GPUI.exe"; WorkingDir: "{app}"; IconFilename: "{app}\oxyde.ico"; AppUserModelID: "Oxyde.GPUI.Windows"; Tasks: desktopicon

[Run]
Filename: "{app}\Oxyde_GPUI.exe"; Description: "{cm:LaunchProgram,Oxyde GPUI}"; Flags: nowait postinstall skipifsilent
