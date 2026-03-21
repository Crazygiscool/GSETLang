@echo off
setlocal enabledelayedexpansion

REM GSET Windows Installer Script (No external tools needed)
REM Run as: install.bat

echo ========================================
echo   GSET Installer for Windows
echo ========================================
echo.

set "INSTALL_DIR=%ProgramFiles%\GSET"
set "BINARY_NAME=gset.exe"

REM Check for admin rights
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo ERROR: Please run as Administrator
    echo Right-click and select "Run as administrator"
    pause
    exit /b 1
)

echo Installing GSET to %INSTALL_DIR%...
echo.

REM Create installation directory
if not exist "%INSTALL_DIR%" (
    mkdir "%INSTALL_DIR%"
)

REM Copy binary (assuming it's in current directory)
if exist "gset.exe" (
    copy /Y "gset.exe" "%INSTALL_DIR%\" >nul
    echo [OK] Binary installed
) else (
    echo [WARN] gset.exe not found in current directory
    echo        Please place gset.exe in the same folder as this script
)

REM Copy config file
if exist "gset.conf" (
    copy /Y "gset.conf" "%INSTALL_DIR%\" >nul
    echo [OK] Config file installed
)

REM Add to PATH
set "PATH_REG=HKLM\System\CurrentControlSet\Control\Session Manager\Environment"
for /f "tokens=2*" %%a in ('reg query "%PATH_REG%" /v Path 2^>nul') do set "CURRENT_PATH=%%b"

echo %CURRENT_PATH% | findstr /C:"%INSTALL_DIR%" >nul
if %errorlevel% neq 0 (
    setx PATH "%CURRENT_PATH%;%INSTALL_DIR%" >nul
    echo [OK] Added to system PATH
) else (
    echo [OK] Already in PATH
)

echo.
echo ========================================
echo   Installation Complete!
echo ========================================
echo.
echo GSET has been installed to: %INSTALL_DIR%
echo.
echo To use in any terminal, just type: gset ^<filename^>
echo.
echo To verify installation, run: gset --version
echo.

REM Create uninstaller
(
echo @echo off
echo echo Uninstalling GSET...
echo del /Q "%INSTALL_DIR%\gset.exe" 2^>nul
echo del /Q "%INSTALL_DIR%\gset.conf" 2^>nul
echo rmdir "%INSTALL_DIR%" 2^>nul
echo echo GSET has been uninstalled.
echo pause
) > "%INSTALL_DIR%\uninstall.bat"

echo Press any key to exit...
pause >nul