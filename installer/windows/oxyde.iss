; The GPUI identity is independent of the Tauri one; the Tauri install is removed after setup.
#ifndef AppVersion
  #error AppVersion must be provided by scripts/build-distribution.ps1
#endif
#ifndef PayloadDir
  #error PayloadDir must be provided by scripts/build-distribution.ps1
#endif

[Setup]
AppId=Oxyde.GPUI.Windows
AppName=Oxyde
AppVersion={#AppVersion}
AppPublisher=Carlton2001
DefaultDirName={localappdata}\Programs\Oxyde_FM
DefaultGroupName=Oxyde
DisableProgramGroupPage=yes
DisableDirPage=no
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
UninstallDisplayName=Oxyde
UninstallDisplayIcon={app}\oxyde.ico
SetupIconFile={#PayloadDir}\oxyde.ico
OutputBaseFilename=Oxyde_FM_{#AppVersion}_x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ShowLanguageDialog=auto
; The in-app updater exits before launching Setup; never close other processes.
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
Source: "{#PayloadDir}\Oxyde_FM.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\oxyde-image-worker.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\oxyde-archive-io-worker.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\oxyde-archive-list-worker.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\oxyde.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\*.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Oxyde"; Filename: "{app}\Oxyde_FM.exe"; WorkingDir: "{app}"; IconFilename: "{app}\oxyde.ico"; AppUserModelID: "Oxyde.GPUI.Windows"
Name: "{autodesktop}\Oxyde"; Filename: "{app}\Oxyde_FM.exe"; WorkingDir: "{app}"; IconFilename: "{app}\oxyde.ico"; AppUserModelID: "Oxyde.GPUI.Windows"; Tasks: desktopicon

[Run]
Filename: "{app}\Oxyde_FM.exe"; Description: "{cm:LaunchProgram,Oxyde}"; Flags: nowait postinstall skipifsilent; Check: not IsUpdate
Filename: "{app}\Oxyde_FM.exe"; Flags: nowait; Check: IsUpdate

[Code]
// The Tauri updater and the GPUI updater both launch Setup with /UPDATE.
function IsUpdate: Boolean;
var
  Index: Integer;
begin
  Result := False;
  for Index := 1 to ParamCount do
    if CompareText(ParamStr(Index), '/UPDATE') = 0 then
      Result := True;
end;

const
  SYNCHRONIZE = $00100000;

function OpenProcess(Access: DWORD; Inherit: BOOL; ProcessId: DWORD): THandle;
  external 'OpenProcess@kernel32.dll stdcall';
function WaitForSingleObject(Handle: THandle; Milliseconds: DWORD): DWORD;
  external 'WaitForSingleObject@kernel32.dll stdcall';
function CloseHandle(Handle: THandle): BOOL;
  external 'CloseHandle@kernel32.dll stdcall';

// The GPUI updater passes its process id so files are replaced after it exits.
function InitializeSetup: Boolean;
var
  Process: THandle;
begin
  Process := OpenProcess(SYNCHRONIZE, False, StrToIntDef(ExpandConstant('{param:WAITPID|0}'), 0));
  if Process <> 0 then
  begin
    WaitForSingleObject(Process, 60000);
    CloseHandle(Process);
  end;
  Result := True;
end;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := IsUpdate;
end;

// Transitional: remove the Tauri edition (Oxyde_FM 1.x) and its data.
// Drop this section once Tauri installs no longer need migrating.
procedure RemoveTauriInstall(RootKey: Integer; const SubKey: String);
var
  Uninstall: String;
  Code: Integer;
begin
  if not RegQueryStringValue(RootKey, SubKey, 'UninstallString', Uninstall) then
    Exit;
  if Pos('MsiExec.exe', Uninstall) = 1 then
    // Per-machine MSI: request elevation for msiexec only.
    ShellExec('runas', 'msiexec.exe',
      '/x ' + Copy(Uninstall, Pos('{', Uninstall), 38) + ' /passive /norestart',
      '', SW_SHOW, ewWaitUntilTerminated, Code)
  else
    Exec(RemoveQuotes(Uninstall), '/S', '', SW_SHOW, ewWaitUntilTerminated, Code);
end;

procedure RemoveTauriEdition;
var
  Roots: array[0..1] of Integer;
  Keys: TArrayOfString;
  Base, Key, Name, Publisher: String;
  R, K: Integer;
begin
  Base := 'Software\Microsoft\Windows\CurrentVersion\Uninstall';
  Roots[0] := HKCU;
  Roots[1] := HKLM64;
  for R := 0 to 1 do
    if RegGetSubkeyNames(Roots[R], Base, Keys) then
      for K := 0 to GetArrayLength(Keys) - 1 do
      begin
        Key := Base + '\' + Keys[K];
        if RegQueryStringValue(Roots[R], Key, 'DisplayName', Name)
          and RegQueryStringValue(Roots[R], Key, 'Publisher', Publisher)
          and (Name = 'Oxyde_FM') and (Publisher = 'Oxyde') then
          RemoveTauriInstall(Roots[R], Key);
      end;
  DelTree(ExpandConstant('{userappdata}\com.oxyde.desktop'), True, True, True);
  DelTree(ExpandConstant('{localappdata}\com.oxyde.desktop'), True, True, True);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    RemoveTauriEdition;
end;
