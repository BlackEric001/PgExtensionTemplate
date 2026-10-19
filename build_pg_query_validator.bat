@echo off
setlocal EnableExtensions EnableDelayedExpansion
chcp 65001 >nul
title Build pg_query_validator (PostgreSQL 18 / pgrx 0.19.3)

rem ============================================================================
rem Requirements (install these before running this script):
rem   1. Windows 10/11 x64.
rem   2. Rust stable MSVC toolchain (rustup, rustc, cargo).
rem   3. Visual Studio 2022 Build Tools with "Desktop development with C++",
rem      MSVC x64/x86 tools, and Windows 10/11 SDK (including Universal CRT).
rem   4. LLVM/Clang with libclang.dll (11 or newer); this script checks for it.
rem   5. PostgreSQL 18 x64 installed with server headers and pg_config.exe.
rem   6. Internet access on first build so Cargo can download dependencies.
rem
rem Run from any working directory. The script assumes it is in the repository
rem root, next to the pg_query_validator directory.
rem ============================================================================

echo.
echo ============================================================
echo  Build pg_query_validator - PostgreSQL 18 / pgrx 0.19.3
echo ============================================================
echo.

set "SCRIPT_DIR=%~dp0"
set "ROOT=%SCRIPT_DIR%"
set "PROJECT="

rem Support both layouts:
rem   A) BAT in repository root, with pg_query_validator\Cargo.toml
rem   B) BAT inside pg_query_validator, next to Cargo.toml
if exist "%SCRIPT_DIR%Cargo.toml" set "PROJECT=%SCRIPT_DIR%"
if not defined PROJECT if exist "%SCRIPT_DIR%pg_query_validator\Cargo.toml" set "PROJECT=%SCRIPT_DIR%pg_query_validator\"
if not defined PROJECT (
  echo [ERROR] Could not find Cargo.toml beside this script or in a pg_query_validator subfolder.
  echo Place this BAT in the repository root or directly inside the Rust project folder.
  goto :fail
)

set "PGROOT=C:\Program Files\PostgreSQL\18"
set "PGCONFIG=%PGROOT%\bin\pg_config.exe"

echo [1/7] Checking PostgreSQL 18...
if not exist "%PGCONFIG%" (
  echo [ERROR] "%PGCONFIG%" was not found.
  echo Install PostgreSQL 18 or update PGROOT in this .bat file.
  goto :fail
)
for /f "usebackq delims=" %%V in (`"%PGCONFIG%" --version 2^>nul`) do set "PGVER=%%V"
echo       !PGVER!
echo !PGVER! | findstr /r /c:"PostgreSQL 18\." >nul
if errorlevel 1 (
  echo [ERROR] pg_config does not report PostgreSQL 18: !PGVER!
  goto :fail
)
if not exist "%PGROOT%\include\server\postgres.h" (
  echo [ERROR] PostgreSQL server headers were not found:
  echo         "%PGROOT%\include\server\postgres.h"
  goto :fail
)

echo [2/7] Checking Rust...
where rustup >nul 2>&1 || (echo [ERROR] rustup not found. Install Rust from https://rustup.rs/ ^& goto :fail)
where rustc >nul 2>&1 || (echo [ERROR] rustc not found. ^& goto :fail)
where cargo >nul 2>&1 || (echo [ERROR] cargo not found. ^& goto :fail)
rustc -V
cargo -V
rem Check the compiler host using FIND, which performs a literal text match.
rustc -vV | find /c "host: x86_64-pc-windows-msvc" >nul
if errorlevel 1 (
  echo [ERROR] Rust MSVC toolchain x86_64-pc-windows-msvc is required.
  echo Detected Rust compiler information:
  rustc -vV
  echo Run: rustup default stable-x86_64-pc-windows-msvc
  goto :fail
)
echo       Rust host: x86_64-pc-windows-msvc

echo [3/7] Checking Visual Studio C++ Build Tools...
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" (
  echo [ERROR] vswhere.exe not found. Install Visual Studio 2022 Build Tools
  echo         with the "Desktop development with C++" workload.
  goto :fail
)
set "VSROOT="
for /f "usebackq delims=" %%I in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VSROOT=%%I"
if not defined VSROOT (
  echo [ERROR] MSVC x64/x86 Build Tools were not found.
  echo Open Visual Studio Installer and add "Desktop development with C++".
  goto :fail
)
set "DEVCMD=%VSROOT%\Common7\Tools\VsDevCmd.bat"
if not exist "%DEVCMD%" (
  echo [ERROR] "%DEVCMD%" was not found.
  goto :fail
)
call "%DEVCMD%" -arch=x64 -host_arch=x64 >nul
if errorlevel 1 (
  echo [ERROR] Could not initialize the Visual Studio environment.
  goto :fail
)
where cl.exe >nul 2>&1
if errorlevel 1 (
  echo [ERROR] cl.exe was not found after running VsDevCmd.bat.
  goto :fail
)
echo       Visual Studio: !VSROOT!

echo [4/7] Checking CRT/Windows SDK headers...
set "CRTDEFS="
set "CORECRT="
for /f "delims=" %%F in ('where /r "%VSROOT%\VC\Tools\MSVC" crtdefs.h 2^>nul') do if not defined CRTDEFS set "CRTDEFS=%%F"
for /f "delims=" %%F in ('where /r "%ProgramFiles(x86)%\Windows Kits\10\Include" corecrt.h 2^>nul') do if not defined CORECRT set "CORECRT=%%F"
if not defined CRTDEFS (
  echo [ERROR] crtdefs.h was not found in the MSVC directory.
  echo Repair the MSVC C++ installation through Visual Studio Installer.
  goto :fail
)
if not defined CORECRT (
  echo [ERROR] corecrt.h was not found in the Windows SDK UCRT.
  echo Install the Windows 10/11 SDK and Universal CRT component.
  goto :fail
)
echo       crtdefs.h: !CRTDEFS!
echo       corecrt.h: !CORECRT!

echo [5/7] Checking LLVM / libclang...
if not defined LIBCLANG_PATH (
  if exist "C:\Program Files\LLVM\bin\libclang.dll" set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"
  if exist "C:\Program Files (x86)\LLVM\bin\libclang.dll" set "LIBCLANG_PATH=C:\Program Files (x86)\LLVM\bin"
)
if not defined LIBCLANG_PATH (
  echo [ERROR] LIBCLANG_PATH is unset and libclang.dll was not found in standard folders.
  echo Install LLVM from https://github.com/llvm/llvm-project/releases
  echo or set LIBCLANG_PATH to the directory containing libclang.dll.
  goto :fail
)
if not exist "%LIBCLANG_PATH%\libclang.dll" (
  echo [ERROR] libclang.dll not found in LIBCLANG_PATH: "%LIBCLANG_PATH%"
  goto :fail
)
echo       LIBCLANG_PATH=!LIBCLANG_PATH!

echo [6/7] Configuring paths and checking pgrx version...
rem VsDevCmd initializes PATH, INCLUDE and LIB. Add the SDK directories explicitly
rem so libclang/bindgen can find MSVC and Universal CRT headers as well.
set "INCLUDE=%INCLUDE%;%ProgramFiles(x86)%\Windows Kits\10\Include"
for /d %%S in ("%ProgramFiles(x86)%\Windows Kits\10\Include\*") do (
  if exist "%%~S\ucrt\corecrt.h" set "INCLUDE=%INCLUDE%;%%~S\ucrt;%%~S\shared;%%~S\um;%%~S\winrt;%%~S\cppwinrt"
)
for /d %%M in ("%VSROOT%\VC\Tools\MSVC\*") do (
  if exist "%%~M\include\crtdefs.h" set "INCLUDE=%INCLUDE%;%%~M\include;%%~M\atlmfc\include"
)
set "PGRX_PG_CONFIG_PATH=%PGCONFIG%"
findstr /c:"pgrx = \"0.19.3\"" "%PROJECT%\Cargo.toml" >nul
if errorlevel 1 (
  echo [WARN] Exact dependency line pgrx = "0.19.3" was not found in Cargo.toml.
  echo        Check the pgrx dependency manually.
) else (
  echo       pgrx dependency: 0.19.3
)
echo       pg_config: %PGRX_PG_CONFIG_PATH%

echo [7/7] Building release for PostgreSQL 18...
rem Work around Clang diagnostics in PostgreSQL 18 MSVC atomic headers during bindgen.
rem This only affects bindgen parsing; it does not change compiler flags for the extension itself.
if defined BINDGEN_EXTRA_CLANG_ARGS (
  set "BINDGEN_EXTRA_CLANG_ARGS=%BINDGEN_EXTRA_CLANG_ARGS% -Wno-incompatible-pointer-types -Wno-deprecated-declarations"
) else (
  set "BINDGEN_EXTRA_CLANG_ARGS=-Wno-incompatible-pointer-types -Wno-deprecated-declarations"
)
echo       BINDGEN_EXTRA_CLANG_ARGS=%BINDGEN_EXTRA_CLANG_ARGS%
pushd "%PROJECT%"
cargo build --release --locked --features pg18
set "BUILD_RESULT=!ERRORLEVEL!"
popd
if not "!BUILD_RESULT!"=="0" (
  echo.
  echo [ERROR] Cargo build failed with exit code !BUILD_RESULT!.
  echo If the error mentions crtdefs.h again, check INCLUDE and the Windows SDK.
  exit /b !BUILD_RESULT!
)

echo.
echo [OK] Build completed successfully.
echo Rust artifacts: "%PROJECT%\target\release"
echo For a debug build, remove --release.
exit /b 0

:fail
echo.
echo Build stopped. Fix the issue above and run the .bat again.
pause
exit /b 1
