#!/usr/bin/env bash
# Fetch the upstream VPet 0000_core content pack used by the Android build.
#
# The artwork is not part of this repository. This script mirrors what the
# desktop VPetRS build does: reuse a local VPet installation when possible,
# otherwise download only the content folder from the upstream repository.
set -euo pipefail

DEST="${VPET_CORE_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/vpet-rs/content/0000_core}"
FROM=""
FORCE=0

usage() {
    cat <<'EOF'
Usage: scripts/fetch-assets.sh [options]

Options:
  --from PATH   Use an existing VPet content folder (the one containing info.lps
                and pet/vup), for example your VPet installation:
                .../VPet-Simulator.Windows/mod/0000_core
  --dest PATH   Where to place or link the content folder.
                Default: ${XDG_DATA_HOME:-~/.local/share}/vpet-rs/content/0000_core
  --force       Replace an existing destination.
  -h, --help    Show this help.

Resolution order when --from is not given:
  1. An existing, valid destination.
  2. A local Steam VPet installation.
  3. A sparse git checkout of LorisYounger/VPet (assets only, ~1.1 GB).
EOF
}

die() {
    echo "error: $*" >&2
    exit 1
}

is_valid_core() {
    [[ -d "$1" && -f "$1/info.lps" && -d "$1/pet/vup" ]]
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --from)
            [[ $# -ge 2 ]] || die "--from requires a path"
            FROM="$2"
            shift 2
            ;;
        --dest)
            [[ $# -ge 2 ]] || die "--dest requires a path"
            DEST="$2"
            shift 2
            ;;
        --force)
            FORCE=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            die "unknown option: $1 (see --help)"
            ;;
    esac
done

if [[ $FORCE -eq 1 ]]; then
    if [[ -L "$DEST" ]]; then
        rm -f "$DEST"
    elif [[ -e "$DEST" ]]; then
        rm -rf "$DEST"
    fi
fi

link_core() {
    local source="$1"
    mkdir -p "$(dirname "$DEST")"
    ln -sfn "$source" "$DEST"
    echo "Linked VPet assets: $DEST -> $source"
}

if [[ -n "$FROM" ]]; then
    is_valid_core "$FROM" || die "not a VPet 0000_core folder: $FROM"
    link_core "$FROM"
    exit 0
fi

if is_valid_core "$DEST"; then
    echo "VPet assets already available at: $DEST"
    exit 0
fi

for candidate in \
    "$HOME/.local/share/Steam/steamapps/common/VPet/mod/0000_core" \
    "$HOME/.steam/steam/steamapps/common/VPet/mod/0000_core" \
    "$HOME/.steam/root/steamapps/common/VPet/mod/0000_core" \
    "${VPET_INSTALL:-/nonexistent}/VPet-Simulator.Windows/mod/0000_core"; do
    if is_valid_core "$candidate"; then
        link_core "$candidate"
        exit 0
    fi
done

command -v git >/dev/null 2>&1 || die "git is required to download the assets"

echo "Downloading VPet 0000_core via git sparse-checkout (~1.1 GB, assets only)..."
TMP="$(mktemp -d -t vpet-mobile-assets-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT

git clone --depth 1 --filter=blob:none --sparse \
    https://github.com/LorisYounger/VPet.git "$TMP/vpet"
git -C "$TMP/vpet" sparse-checkout set VPet-Simulator.Windows/mod/0000_core

SOURCE="$TMP/vpet/VPet-Simulator.Windows/mod/0000_core"
is_valid_core "$SOURCE" || die "upstream checkout did not contain 0000_core"

mkdir -p "$(dirname "$DEST")"
# Copy instead of move so the temporary checkout is removed by the trap.
cp -a "$SOURCE" "$DEST"

echo "Installed VPet assets at: $DEST"
