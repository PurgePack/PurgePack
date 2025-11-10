#!/usr/bin/env bash
set -euo pipefail

DO_CARGO=0
DO_PURGEPACK=0
CARGO_ARGS=()
PURGEPACK_ARGS=()

while [[ $# -gt 0 ]]; do
    arg="$1"

    case "$arg" in
        +cargo)
            DO_CARGO=1
            shift
            while [[ $# -gt 0 && "$1" != +* ]]; do
                CARGO_ARGS+=("$1")
                shift
            done
            ;;
        +run)
            DO_PURGEPACK=1
            shift
            while [[ $# -gt 0 && "$1" != +* ]]; do
                PURGEPACK_ARGS+=("$1")
                shift
            done
            ;;
        *)
            echo "Warning: Argument '$arg' outside any section - ignoring"
            shift
            ;;
    esac
done

if [[ "$DO_CARGO" -eq 1 ]]; then
    echo "Running cargo ${CARGO_ARGS[*]}"
    if ! cargo "${CARGO_ARGS[@]}"; then
        echo "cargo failed, exiting"
        exit 1
    fi

    if printf '%s\n' "${CARGO_ARGS[@]}" | grep -q "build"; then
        if printf '%s\n' "${CARGO_ARGS[@]}" | grep -q "release"; then
            cd target/release
        else
            cd target/debug
        fi
    fi

    if [[ ! -d modules ]]; then
        echo "Creating modules folder"
        mkdir -p modules
    fi

    for f in *.so; do
        if [[ -f "$f" ]]; then
            echo "Moving $f to modules folder"
            mv "$f" modules/
        fi
    done

    echo "BUILD FINISHED"
fi

if [[ "$DO_PURGEPACK" -eq 1 ]]; then
    echo "Running ./purgepack ${PURGEPACK_ARGS[*]}"
    if ! ./purgepack "${PURGEPACK_ARGS[@]}"; then
        echo "purgepack failed, exiting"
        exit 1
    fi
fi

exit 0
