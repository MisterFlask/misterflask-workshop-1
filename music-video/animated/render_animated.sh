#!/bin/bash
# Assemble the animated music video: frames + lyric overlay + audio.
# Usage: FRAMES=<dir> AUDIO=<song.mp3> ./render_animated.sh out.mp4
set -e
FRAMES="${FRAMES:-frames}"
AUDIO="${AUDIO:-song.mp3}"
OUT="${1:-stokers_animated.mp4}"

ffmpeg -y -loglevel warning -stats \
  -framerate 24 -i "$FRAMES/f%05d.png" \
  -i "$AUDIO" \
  -filter_complex "[0:v]format=yuv420p,ass=../lyrics.ass:fontsdir=/usr/share/fonts/truetype/dejavu[v]" \
  -map "[v]" -map 1:a \
  -c:v libx264 -preset medium -crf 21 -pix_fmt yuv420p \
  -c:a aac -b:a 160k \
  -shortest -movflags +faststart \
  "$OUT"
