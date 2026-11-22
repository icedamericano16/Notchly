; Notchly direct installer (Inno Setup 6). Per-user, no admin.
#ifndef AppVersion
  #define AppVersion "0.1.3"
#endif
#ifndef ExeDir
  #define ExeDir "..\..\target\release"
#endif

[Setup]
AppId={{6E0F5D9A-3C1B-4F7E-9B2A-4C6D8E1F0A31}
AppName=Notchly
AppVersion={#AppVersion}
AppPublisher=Notchly
#ifdef AppURL
AppPublisherURL={#AppURL}
#endif
LicenseFile=..\..\LICENSE
UninstallDisplayName=Notchly
DefaultDirName={localappdata}\Programs\Notchly
DefaultGroupName=Notchly
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputBaseFilename=Notchly-{#AppVersion}-setup
OutputDir=..\..\dist
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.19041
UninstallDisplayIcon={app}\notchly.exe
CloseApplications=force
RestartApplications=yes

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked
Name: "autostart"; Description: "Start Notchly when I sign in"
Name: "notifications"; Description: "Enable notification mirroring (registers a small identity package)"

[Files]
Source: "{#ExeDir}\notchly.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\dist\Notchly.Sparse.msix"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{autoprograms}\Notchly"; Filename: "{app}\notchly.exe"
Name: "{autodesktop}\Notchly"; Filename: "{app}\notchly.exe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Notchly"; ValueData: """{app}\notchly.exe"" --autostart"; Flags: uninsdeletevalue; Tasks: autostart
; CLI: put notchly.exe on the user PATH so `notchly live ...` works in terminals
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))

[Run]
; register the sparse identity package (signed) so notifications can be mirrored
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -Command ""Add-AppxPackage -Path '{app}\Notchly.Sparse.msix' -ExternalLocation '{app}'"""; Flags: runhidden; Tasks: notifications; Check: FileExists(ExpandConstant('{app}\Notchly.Sparse.msix'))
Filename: "{app}\notchly.exe"; Description: "Launch Notchly"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "powershell.exe"; Parameters: "-NoProfile -Command ""Get-AppxPackage Notchly.Sparse | Remove-AppxPackage"""; Flags: runhidden; RunOnceId: "RemoveSparse"
Filename: "taskkill.exe"; Parameters: "/F /IM notchly.exe"; Flags: runhidden; RunOnceId: "KillNotchly"

[Code]
function NeedsAddPath(Param: string): boolean;
var OrigPath: string;
begin
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', OrigPath) then begin Result := True; exit; end;
  Result := Pos(';' + Uppercase(Param) + ';', ';' + Uppercase(OrigPath) + ';') = 0;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    if MsgBox('Also remove your Notchly settings and data?', mbConfirmation, MB_YESNO) = IDYES then
      DelTree(ExpandConstant('{userappdata}\Notchly'), True, True, True);
end;
