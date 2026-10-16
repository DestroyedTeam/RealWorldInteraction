#!/bin/bash
# Operator-only local packaging. Does not change TCC, install, or restart services.
set -euo pipefail
if [[ $# -ne 2 ]]; then
  echo 'Usage: sign_rwi_macos_app.sh /absolute/RWI\ Agent.app CERTIFICATE_SHA1' >&2
  exit 2
fi
app=$1
identity=$2
[[ "$app" = /* && "$app" = *.app && -d "$app/Contents/MacOS" ]] || exit 2
[[ "$identity" =~ ^[[:xdigit:]]{40}$ ]] || { echo 'An explicit certificate SHA1 is required; ad-hoc fallback is forbidden.' >&2; exit 2; }
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Contents/Info.plist")" = com.destroyedteam.real-world-interaction.agent ]] || exit 2
root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$app/Contents/Resources"
cp "$root/integrations/macos/assets/RWI.icns" "$app/Contents/Resources/RWI.icns"
/usr/libexec/PlistBuddy -c 'Set :CFBundleIconFile RWI.icns' "$app/Contents/Info.plist" 2>/dev/null || /usr/libexec/PlistBuddy -c 'Add :CFBundleIconFile string RWI.icns' "$app/Contents/Info.plist"
/usr/bin/codesign --force --sign "$identity" --timestamp=none "$app"
/usr/bin/codesign --verify --strict "$app"
/usr/bin/codesign -d -r- "$app"
