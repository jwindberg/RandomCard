#!/bin/bash
# Builds RandomCard.app from this checkout and puts it in /Applications,
# as the release workflow does but for this Mac's own processor only.
#
#   tools/make_mac_app.sh            # into /Applications
#   tools/make_mac_app.sh ~/Applications
#
# It signs the app with this Mac's Developer ID or Apple Development
# identity (or RANDOMCARD_SIGN_IDENTITY), so the Keychain knows each new
# build as the same app: "Always Allow" for a connection's password then
# lasts. With none, it signs ad hoc, and the Keychain asks again after
# each build.
#
# Written for the bash 3.2 that comes with macOS.
set -euo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
    echo "make_mac_app.sh makes a Mac app, so it runs on a Mac." >&2
    exit 1
fi

cd "$(dirname "$0")/.."
dest="${1:-/Applications}"
version=$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -n 1)

cargo build --release --locked

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

app="$work/RandomCard.app/Contents"
mkdir -p "$app/MacOS" "$app/Resources"
cp target/release/randomcard "$app/MacOS/RandomCard"
sed "s/VERSION/$version/g" packaging/macos/Info.plist > "$app/Info.plist"

cargo run --release --locked --example make_icons -- "$work/icons"
iconset="$work/RandomCard.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
    cp "$work/icons/icon_$size.png" "$iconset/icon_${size}x${size}.png"
    cp "$work/icons/icon_$((size * 2)).png" "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$iconset" -o "$app/Resources/RandomCard.icns"

identity="${RANDOMCARD_SIGN_IDENTITY:-}"
if [ -z "$identity" ]; then
    identities=$(security find-identity -v -p codesigning 2>/dev/null || true)
    for kind in "Developer ID Application" "Apple Development"; do
        identity=$(printf '%s\n' "$identities" | grep -o "\"$kind: [^\"]*\"" | head -n 1 | tr -d '"' || true)
        [ -n "$identity" ] && break
    done
fi
if [ -n "$identity" ]; then
    codesign --force --sign "$identity" "$work/RandomCard.app"
    echo "Signed by $identity"
else
    codesign --force --sign - "$work/RandomCard.app"
    echo "No signing identity: signed ad hoc, so the Keychain asks again after each build" >&2
fi

mkdir -p "$dest"
rm -rf "$dest/RandomCard.app"
ditto "$work/RandomCard.app" "$dest/RandomCard.app"
# So Finder and Launchpad notice the new version and its icon.
touch "$dest/RandomCard.app"

echo "RandomCard $version is in $dest/RandomCard.app"
