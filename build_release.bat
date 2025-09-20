@echo off

set "binaryName=purgepack.exe"
set "outputDir=target/"

cargo build --release
if %errorlevel% neq 0 (
    echo Cargo build failed. Exiting.
    exit /b 1
)

if not exist "%outputDir%" mkdir "%outputDir%"

move "target\release\%binaryName%" "%outputDir%"

echo Build and move complete. Binary is in %outputDir%
