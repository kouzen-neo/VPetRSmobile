#!/usr/bin/env bash
set -euo pipefail

APK_PATH="${1:-/home/kouzen/Documents/VPetRustMobile/vpet-mobile.apk}"
DEVICE_ID="0b53e7a3ff7348e39f76261497e740b6"

if [ ! -f "$APK_PATH" ]; then
    echo "Error: APK not found at $APK_PATH" >&2
    exit 1
fi

echo "Refreshing KDE Connect devices..."
kdeconnect-cli --refresh
sleep 1

for i in $(seq 1 10); do
    if kdeconnect-cli -a | grep -q "$DEVICE_ID"; then
        echo "POCO F5 reachable! Sending $(basename "$APK_PATH") (${APK_PATH})..."
        kdeconnect-cli -d "$DEVICE_ID" --share "$APK_PATH"
        echo "Transfer initiated successfully!"
        exit 0
    fi
    echo "Attempt $i/10: Waiting for POCO F5 to be reachable on LAN... (Make sure phone screen is ON and KDE Connect is open)"
    sleep 2
done

echo "POCO F5 not reachable yet. Please unlock phone and open KDE Connect app, then run:"
echo "kdeconnect-cli -d $DEVICE_ID --share \"$APK_PATH\""
exit 1
