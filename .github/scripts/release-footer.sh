#!/usr/bin/env bash
# The footer of every release's notes: a marker line, a rule, then how to install and verify. Both ways of
# starting a release (release-current and release-plz) use it, so there is one copy of the text.
# Everything above the marker line is the release's own notes (release.yml stores those in the packages).
#
# Usage: release-footer.sh <owner/repo>
set -euo pipefail

repo=$1
id=AndrewKorchemniy.PRMonitor

echo '<!-- release-footer -->'
echo '---'
echo
echo 'Builds are not code-signed, so Windows SmartScreen may warn on first run (More info > Run anyway).'
echo
echo "**Install:** run \`$id-win-x64-Setup.exe\` (or \`-win-arm64\`); it installs for your user only and needs no admin rights. Or unzip \`$id-win-x64-Portable.zip\` anywhere and run \`PR Monitor.exe\`. Both update themselves."
echo
echo "**Linux (preview):** download \`$id-linux-x64.AppImage\` (or \`-linux-arm64\`), \`chmod +x\` it and run it; it updates itself. Needs libxkbcommon, X11 or Wayland and GL, and a StatusNotifier host for the tray icon; see the README."
echo
echo "Verify: \`gh attestation verify $id-win-x64-Setup.exe -R $repo\` · checksums in \`SHA256SUMS.txt\` · PDBs attached for crash symbolization."
