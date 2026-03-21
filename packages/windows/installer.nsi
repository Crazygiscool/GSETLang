; GSET NSIS Installer Script
; Run with: makensis installer.nsi

!include "MUI2.nsh"

; General
Name "GSET"
OutFile "gset-setup.exe"
InstallDir "$PROGRAMFILES64\GSET"
RequestExecutionLevel admin

; Interface Settings
!define MUI_ABORTWARNING

; Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Language
!insertmacro MUI_LANGUAGE "English"

; Install Section
Section "Install"
    SetOutPath "$INSTDIR"
    
    ; Copy files
    File "dist\gset-windows-amd64.exe"
    
    ; Create uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"
    
    ; Create start menu shortcuts
    CreateDirectory "$SMPROGRAMS\GSET"
    CreateShortCut "$SMPROGRAMS\GSET\GSET.lnk" "$INSTDIR\gset.exe"
    CreateShortCut "$SMPROGRAMS\GSET\Uninstall.lnk" "$INSTDIR\uninstall.exe"
    
    ; Create desktop shortcut
    CreateShortCut "$DESKTOP\GSET.lnk" "$INSTDIR\gset.exe"
    
    ; Add to PATH
    WriteRegStr HKLM "System\CurrentControlSet\Control\Session Manager\Environment" "PATH" "$PATH;$INSTDIR"
    
    ; Write uninstall info
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "DisplayName" "GSET"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "UninstallString" "$INSTDIR\uninstall.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "InstallLocation" "$INSTDIR"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "DisplayIcon" "$INSTDIR\gset.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "Publisher" "GSET Team"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "DisplayVersion" "1.0.0"
SectionEnd

; Uninstall Section
Section "Uninstall"
    ; Remove files
    Delete "$INSTDIR\gset.exe"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"
    
    ; Remove shortcuts
    Delete "$SMPROGRAMS\GSET\GSET.lnk"
    Delete "$SMPROGRAMS\GSET\Uninstall.lnk"
    RMDir "$SMPROGRAMS\GSET"
    Delete "$DESKTOP\GSET.lnk"
    
    ; Remove from PATH (simplified - would need more logic for production)
SectionEnd