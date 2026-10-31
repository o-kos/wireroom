#!/usr/bin/env bash
# Make a local app bundle after cargo build --release; no installation or signing.
set -euo pipefail
root=$(git -C "$(dirname -- "$0")" rev-parse --show-toplevel)
bundle="$root/target/Wireroom.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
install -m755 "$root/target/release/wr" "$bundle/Contents/MacOS/wr"
install -m644 "$root/crates/ui/assets/wireroom.icns" "$bundle/Contents/Resources/wireroom.icns"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>wr</string>
<key>CFBundleIdentifier</key><string>io.github.o_kos.wireroom</string>
<key>CFBundleName</key><string>Wireroom</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleIconFile</key><string>wireroom</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
