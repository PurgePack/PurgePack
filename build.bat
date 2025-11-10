@echo off
setlocal EnableDelayedExpansion

set all=%*

set run=%1
if defined run shift

if "%run%"=="--run" (
    set state=%1
    set args=%2
)
:: remove surrounding quotes from purgepack args
if "%run%"=="--run" and "%args:~0,1%"==^" set args=%args:~1,-1%
if not "%args:~0,1%"==^" set args=""

if defined run shift
if not "%args%"=="" shift
if "%run%"=="--release" (
    set state=--release
    shift
)

set RESTVAR=
shift
:loop1
if "%1"=="" goto after_loop
set RESTVAR=%RESTVAR% %1
shift
goto loop1
:after_loop

if "%state%"=="--release" (
    echo Building in release mode
    if "%run%" == "" (
        cargo build --release %all%
    ) else (
        cargo build --release %RESTVAR%
    )
    cd .\target\release
) else (
    echo Building in debug mode
    if "%run%" == "" (
        cargo build %all%
    ) else (
        cargo build %RESTVAR%
    )
    cd .\target\debug
)

for %%f in (*.dll) do (
    if "%%~xf"==".dll" (
        if not exist ".\modules\" (
            echo Creating modules folder
            mkdir ".\modules\"
        )
        echo Moving %%f to modules folder
        move "%%f" .\modules\
    )
)

if "%run%"=="--run" (
    echo Running purgepack.exe
    .\purgepack.exe %args%
)
