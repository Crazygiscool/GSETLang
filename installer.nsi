; GSET NSIS Installer Script
; Builds a proper Windows installer for winget

!include "MUI2.nsh"

; General
Name "GSET"
OutFile "gset-setup.exe"
InstallDir "$PROGRAMFILES64\GSET"
InstallDirRegKey HKLM "Software\GSET" "InstallDir"
RequestExecutionLevel admin

; Version info
VIProductVersion "2.1.2.0"
VIAddVersionKey "ProductName" "GSET"
VIAddVersionKey "CompanyName" "GSETLang"
VIAddVersionKey "LegalCopyright" "Copyright 2024 GSETLang"
VIAddVersionKey "FileDescription" "GSET Installer"
VIAddVersionKey "FileVersion" "2.1.2"
VIAddVersionKey "ProductVersion" "2.1.2"

; Interface Settings
!define MUI_ABORTWARNING
!define MUI_ICON "${NSISDIR}\Contrib\Graphics\Icons\modern-install.ico"
!define MUI_UNICON "${NSISDIR}\Contrib\Graphics\Icons\modern-uninstall.ico"

; Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Languages
!insertmacro MUI_LANGUAGE "English"

; Installer Section
Section "Install"
    SetOutPath "$INSTDIR"
    
    ; Install files
    File "dist\gset-windows-amd64\gset.exe"
    
    ; Create uninstaller
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    
    ; Create Start Menu shortcuts
    CreateDirectory "$SMPROGRAMS\GSET"
    CreateShortcut "$SMPROGRAMS\GSET\GSET.lnk" "$INSTDIR\gset.exe"
    CreateShortcut "$SMPROGRAMS\GSET\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
    
    ; Desktop shortcut
    CreateShortcut "$DESKTOP\GSET.lnk" "$INSTDIR\gset.exe"
    
    ; Registry keys for Add/Remove Programs
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "DisplayName" "GSET"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "InstallLocation" "$INSTDIR"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "Publisher" "GSETLang"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "DisplayVersion" "2.1.2"
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "NoModify" 1
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET" "NoRepair" 1
    
    ; Save install dir
    WriteRegStr HKLM "Software\GSET" "InstallDir" "$INSTDIR"
    
    ; Add to PATH
    ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
    WriteRegStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$0;$INSTDIR"
    
    ; Refresh environment
    System::Call 'Shell32::SHChangeNotify(i 0x8000000, i 0, i 0, i 0)'
SectionEnd

; Uninstaller Section
Section "Uninstall"
    ; Remove files
    Delete "$INSTDIR\gset.exe"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR"
    
    ; Remove shortcuts
    Delete "$SMPROGRAMS\GSET\GSET.lnk"
    Delete "$SMPROGRAMS\GSET\Uninstall.lnk"
    RMDir "$SMPROGRAMS\GSET"
    Delete "$DESKTOP\GSET.lnk"
    
    ; Remove registry keys
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\GSET"
    DeleteRegKey HKLM "Software\GSET"
    
    ; Remove from PATH
    ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
    StrCpy $1 "$INSTDIR;"
    StrCpy $0 "$0|$1" "" "$1"
    WriteRegStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$0"
    
    ; Refresh environment
    System::Call 'Shell32::SHChangeNotify(i 0x8000000, i 0, i 0, i 0)'
SectionEnd
