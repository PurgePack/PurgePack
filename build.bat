@echo off
setlocal enabledelayedexpansion

set DO_CARGO=0
set DO_PURGEPACK=0
set "CARGO_ARGS="
set "PURGEPACK_ARGS="

:parse_loop
set "arg=%~1"
if "%arg%"=="" goto after_parse

if "%arg:~0,1%"=="@" (
    if /I "%arg%"=="@cargo" (
        set DO_CARGO=1
        shift
        goto parse_cargo
    ) else if /I "%arg%"=="@run" (
        set DO_PURGEPACK=1
        shift
        goto parse_purgepack
    ) else (
        echo Warning: Unknown section marker "%arg%" - ignoring
        shift
        goto parse_loop
    )
)

echo Warning: Argument "%arg%" outside any section - ignoring
shift
goto parse_loop

:parse_cargo
set "arg=%~1"
if "%arg%"=="" goto after_parse
if "%arg:~0,1%"=="@" (
    goto parse_loop
)
set "CARGO_ARGS=!CARGO_ARGS! %arg%"
shift
goto parse_cargo

:parse_purgepack
set "arg=%~1"
if "%arg%"=="" goto after_parse
if "%arg:~0,1%"=="@" (
    goto parse_loop
)
set "PURGEPACK_ARGS=!PURGEPACK_ARGS! %arg%"
shift
goto parse_purgepack

:after_parse
if %DO_CARGO%==1 (
    cargo %CARGO_ARGS%

    if not "%CARGO_ARGS:build=%"=="%CARGO_ARGS%" (
        if not "%CARGO_ARGS:release=%"=="%CARGO_ARGS%" (
            cd .\target\release
        ) else (
            cd .\target\debug
        )
    )

    if not exist ".\modules\" (
        echo Creating modules folder
        mkdir ".\modules\"
    )

    for %%f in (*.dll) do (
        if "%%~xf"==".dll" (
            echo Moving %%f to modules folder
            move "%%f" .\modules\
        )
    )
    
    if errorlevel 1 (
        echo cargo failed, exiting
        exit /b %errorlevel%
    )

    echo BUILD FINISHED
)

if %DO_PURGEPACK%==1 (
    .\purgepack.exe %PURGEPACK_ARGS%

    if errorlevel 1 (
        echo purgepack failed, exiting
        exit /b %errorlevel%
    )
)

exit /b 0
