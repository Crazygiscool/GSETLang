; GSET Windows Installer Script
; Version: 2.0.2
; License: CC BY-NC 4.0
; 
; To compile this installer:
; 1. Download Inno Setup from https://jrsoftware.org/isinfo.php
; 2. Open this file in Inno Setup Compiler
; 3. Press Ctrl+F9 to compile
; 4. Output: gset-setup.exe

#define MyAppName "GSET"
#define MyAppVersion "2.0.2"
#define MyAppPublisher "GSET Team"
#define MyAppURL "https://github.com/Crazygiscool/GSETLang"
#define MyAppExeName "gset.exe"

[Setup]
; NOTE: The value of AppId uniquely identifies this application.
AppId={{B5A7D8E2-3F6C-4A8B-9D1E-2C3F4E5A6B7D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=gset-setup-{#MyAppVersion}
Compression=lzma
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\gset.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\gset.conf"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}"

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    // Add to PATH
    if MsgBox('Do you want to add GSET to your system PATH?', mbConfirmation, MB_YESNO) = IDYES then
    begin
      Exec('cmd.exe', '/c setx PATH "%PATH%;' + ExpandConstant('{app}') + '"', '', SW_HIDE, ewWaitUntilTerminated, Result);
    end;
  end;
end;