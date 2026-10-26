#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FORBIDDEN_TREE='dotnet|mono|avalonia|coreclr'
FORBIDDEN_SO='coreclr|mono|dotnet'

if [ -f "$ROOT/Cargo.toml" ]; then
    if cargo tree --workspace 2>/dev/null | grep -Eiq "$FORBIDDEN_TREE"; then
        echo "FAIL: forbidden .NET/Avalonia crate in cargo tree" >&2
        cargo tree --workspace | grep -Ei "$FORBIDDEN_TREE" >&2 || true
        exit 1
    fi
fi

# Check built native shared libraries if present
FOUND_SO=0
for so in $(find "$ROOT/target" -type f -name "libvpet_*.so" 2>/dev/null); do
    FOUND_SO=1
    if file "$so" | grep -qv 'ELF'; then
        echo "FAIL: $so is not an ELF binary" >&2
        exit 1
    fi
    if strings "$so" | grep -Eiq "$FORBIDDEN_SO"; then
        echo "FAIL: $so references forbidden .NET symbols" >&2
        exit 1
    fi
done

echo "OK: zero .NET / Mono dependencies verified"
