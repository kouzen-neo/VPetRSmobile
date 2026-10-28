#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FORBIDDEN_TREE='dotnet|mono|avalonia|coreclr'
FORBIDDEN_NEEDED='coreclr|mono|dotnet|mscorlib|hostfxr'

if [ -f "$ROOT/Cargo.toml" ]; then
    if cargo tree --workspace 2>/dev/null | grep -Eiq "$FORBIDDEN_TREE"; then
        echo "FAIL: forbidden .NET/Avalonia crate in cargo tree" >&2
        cargo tree --workspace | grep -Ei "$FORBIDDEN_TREE" >&2 || true
        exit 1
    fi
fi

# Check built native shared libraries if present
for so in $(find "$ROOT/target" -type f -name "libvpet_*.so" 2>/dev/null); do
    if file "$so" | grep -qv 'ELF'; then
        echo "FAIL: $so is not an ELF binary" >&2
        exit 1
    fi
    if command -v readelf >/dev/null 2>&1; then
        if readelf -d "$so" 2>/dev/null | grep -i 'NEEDED' | grep -Eiq "$FORBIDDEN_NEEDED"; then
            echo "FAIL: $so links against forbidden .NET libraries" >&2
            readelf -d "$so" | grep -i 'NEEDED' >&2
            exit 1
        fi
    fi
done

echo "OK: zero .NET / Mono dependencies verified"
