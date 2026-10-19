@echo off
setlocal EnableExtensions
title Install pg_query_validator for PostgreSQL 18

echo.
echo ==========================================
echo   pg_query_validator - PostgreSQL 18
echo ==========================================
echo.

rem --- Determine paths relative to this BAT file ---
set "REPO_DIR=%~dp0"
rem Remove trailing backslash
if "%REPO_DIR:~-1%"=="\" set "REPO_DIR=%REPO_DIR:~0,-1%"

set "CRATE_DIR=%REPO_DIR%\pg_query_validator"
set "PG_CONFIG=C:\Program Files\PostgreSQL\18\bin\pg_config.exe"
set "VSWHERE=C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"

if not exist "%CRATE_DIR%\Cargo.toml" (
    echo [ERROR] Cargo.toml not found:
    echo "%CRATE_DIR%\Cargo.toml"
    goto :fail
)

pushd "%CRATE_DIR%" || goto :fail

if not exist "%PG_CONFIG%" (
    echo [ERROR] PostgreSQL pg_config.exe not found.
    goto :fail_pop
)

if not exist "pg_query_validator.control" (
    echo [ERROR] Missing pg_query_validator.control
    goto :fail_pop
)

where cargo >nul 2>&1
if errorlevel 1 (
    echo [ERROR] cargo not found in PATH.
    goto :fail_pop
)

echo [INFO] Looking for Visual Studio C++ tools...

if not exist "%VSWHERE%" (
    echo [ERROR] vswhere.exe not found.
    echo Install Visual Studio Build Tools with C++ build tools,
    echo or update this script with your Visual Studio installation path.
    goto :fail_pop
)

set "VSROOT="
for /f "usebackq delims=" %%V in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do (
    set "VSROOT=%%V"
)

if not defined VSROOT (
    echo [ERROR] Visual Studio C++ x64/x86 tools were not found.
    echo Open Visual Studio Installer and install:
    echo Desktop development with C++
    goto :fail_pop
)

set "VSDEVCMD=%VSROOT%\Common7\Tools\VsDevCmd.bat"

if not exist "%VSDEVCMD%" (
    echo [ERROR] VsDevCmd.bat not found:
    echo "%VSDEVCMD%"
    goto :fail_pop
)

echo [INFO] Visual Studio: "%VSROOT%"
echo [INFO] Initializing MSVC x64 environment...

call "%VSDEVCMD%" -arch=amd64 -host_arch=amd64
if errorlevel 1 (
    echo [ERROR] Failed to initialize Visual Studio environment.
    goto :fail_pop
)

echo.
echo [INFO] Checking CRT header...

set "CRT_FOUND="
for /f "tokens=* delims=" %%I in ('where /r "%VSROOT%\VC\Tools\MSVC" crtdefs.h 2^>nul') do (
    set "CRT_FOUND=%%I"
    goto :crt_checked
)

:crt_checked
if not defined CRT_FOUND (
    echo [ERROR] crtdefs.h was not found under:
    echo "%VSROOT%\VC\Tools\MSVC"
    echo Check the MSVC C++ tool installation in Visual Studio Installer.
    goto :fail_pop
)

echo [OK] Found: "%CRT_FOUND%"

echo.
echo [INFO] INCLUDE environment:
echo %INCLUDE%
echo.
echo [INFO] PostgreSQL version:
"%PG_CONFIG%" --version
if errorlevel 1 goto :fail_pop

set "BINDGEN_EXTRA_CLANG_ARGS=-Wno-incompatible-pointer-types -Wno-deprecated-declarations"

echo.
echo [INFO] Running pgrx installation...
echo.

echo.
echo [INFO] Checking PostgreSQL extension DLL...

set "PG_LIB_DIR="
for /f "usebackq delims=" %%I in (`"%PG_CONFIG%" --pkglibdir`) do set "PG_LIB_DIR=%%I"

if exist "%PG_LIB_DIR%\pg_query_validator.dll" (
    echo [INFO] Existing DLL found:
    echo "%PG_LIB_DIR%\pg_query_validator.dll"
    echo [INFO] It will be replaced by cargo pgrx.
    echo If replacement fails with Access denied, stop PostgreSQL
    echo and run this script again as Administrator.
)

cargo pgrx install --release --features pg18 --pg-config "%PG_CONFIG%"
if errorlevel 1 (
    echo.
    echo [ERROR] Installation failed.
    echo If crtdefs.h is still missing, the INCLUDE paths
    echo are not being picked up by libclang.
    goto :fail_pop
)

echo.
echo [OK] Installation completed.
echo Connect to PostgreSQL 18 and run:
echo CREATE EXTENSION pg_query_validator;
echo.

popd
pause
exit /b 0

:fail_pop
popd

:fail
echo.
echo [ERROR] Script stopped.
pause
exit /b 1