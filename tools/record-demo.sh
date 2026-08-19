#!/bin/bash
# Records the account panel and writes the GIF the README shows.
#
#   tools/record-demo.sh            30 seconds
#   tools/record-demo.sh 45         a different length
#
# Put Conductor Dev in fullscreen first, with the green button. The capture is
# whole-screen, so fullscreen is what keeps everything else off it: no dock, no
# menu bar, no other windows, nothing from another project.
#
# Needs Screen Recording permission for the terminal running this, granted in
# System Settings, Privacy & Security, Screen & System Audio Recording.
#
# The panel masks addresses, so a recording of it can be published as it is.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
SECONDS_TO_RECORD="${1:-30}"
RAW=$(mktemp -t hats-demo).mov
OUT="$ROOT/docs/media/account-switching.gif"
WIDTH=1200
FPS=12

die() { echo "record-demo: $*" >&2; exit 1; }

command -v ffmpeg >/dev/null || die "ffmpeg not found: brew install ffmpeg"
mkdir -p "$(dirname "$OUT")"

cat <<EOF
Recording $SECONDS_TO_RECORD seconds. What to show, slowly:

  1. the account button next to "Open in", top right
  2. click it, the panel opens on the provider list
  3. click Claude Code, the accounts appear
  4. click the other account, the tick moves and the badge changes
  5. Back, then close the panel
  6. optionally New Workspace, to show the chip in the composer

EOF

for i in 5 4 3 2 1; do
    printf '\r  starting in %d ' "$i"
    sleep 1
done
printf '\r  recording       \n'

screencapture -v -V "$SECONDS_TO_RECORD" -x "$RAW"

echo "  converting"
PALETTE=$(mktemp -t hats-palette).png
ffmpeg -v error -i "$RAW" -vf "fps=$FPS,scale=$WIDTH:-1:flags=lanczos,palettegen=stats_mode=diff" -y "$PALETTE"
ffmpeg -v error -i "$RAW" -i "$PALETTE" \
    -lavfi "fps=$FPS,scale=$WIDTH:-1:flags=lanczos[v];[v][1:v]paletteuse=dither=bayer:bayer_scale=3" \
    -y "$OUT"
rm -f "$RAW" "$PALETTE"

SIZE=$(du -h "$OUT" | cut -f1)
echo
echo "  $OUT  ($SIZE)"
echo
echo "Watch it before committing. Anything on screen that should not be public"
echo "means delete it and record again; nothing is committed by this script."
echo
echo "To show it in the README, under the opening paragraph:"
echo
echo "  ![Switching accounts from Conductor's toolbar](docs/media/account-switching.gif)"
