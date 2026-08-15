#!/bin/bash
AUDIO="${AUDIO:-song.mp3}"
OUT="${1:-stokers_music_video.mp4}"
SEEK="${2:-}"; DUR="${3:-}"
EXTRA=""
[ -n "$SEEK" ] && EXTRA="-ss $SEEK"
[ -n "$DUR" ] && EXTRA="$EXTRA -t $DUR"

ffmpeg -y -loglevel warning -stats \
  -loop 1 -framerate 30 -i cover.jpg \
  -i "$AUDIO" \
  -filter_complex "
[0:v]format=yuv420p,scale=2304:2304:flags=lanczos,
     crop=1920:1080:x='(iw-1920)/2+90*sin(t/11)':y='(ih-1080)/2+70*cos(t/17)',
     boxblur=14:2,
     eq=brightness=-0.10:saturation=1.25:contrast=1.08,
     vignette=PI/4.2,
     noise=alls=3:allf=t,
     fps=30,format=gbrp[bg];
[1:a]aformat=channel_layouts=mono,
     showwaves=s=1920x240:mode=cline:colors=0xFF7A1A:rate=30:scale=sqrt[w0];
[w0]split[w1][w2];
[w2]gblur=sigma=12[wglow];
[w1][wglow]blend=all_mode=screen,
     pad=1920:1080:0:840:black,format=gbrp[wfull];
[bg][wfull]blend=all_mode=screen,format=yuv420p[withwaves];
[withwaves]ass=lyrics.ass:fontsdir=/usr/share/fonts/truetype/dejavu[v]
" \
  -map "[v]" -map 1:a \
  $EXTRA \
  -c:v libx264 -preset medium -crf 25 -pix_fmt yuv420p \
  -c:a aac -b:a 160k \
  -shortest -movflags +faststart \
  "$OUT"
