#!/usr/bin/env python3
"""Extract embedded lyrics + cover art from an MP3 and parse into sectioned lines.

Suno (and most tagged MP3s) embed unsynced lyrics in the ID3 USLT frame and
cover art in APIC. Section headers like "[Chorus: ...]" become section labels
on each following line.

Usage: parse_lyrics.py <song.mp3>
Writes: lyrics.txt, lines.json, cover.jpg
"""
import json
import re
import sys

from mutagen.id3 import ID3

SECTIONS = ["final chorus", "intro", "verse", "chorus", "bridge", "break", "outro"]


def main(mp3_path):
    tags = ID3(mp3_path)
    lyrics = None
    for key in tags.keys():
        if key.startswith("USLT"):
            lyrics = tags[key].text
            open("lyrics.txt", "w").write(lyrics)
        elif key.startswith("APIC"):
            open("cover.jpg", "wb").write(tags[key].data)
    if lyrics is None:
        sys.exit("no USLT lyrics frame found in " + mp3_path)

    lines = []
    section = "intro"
    for ln in lyrics.splitlines():
        ln = ln.strip()
        if not ln:
            continue
        if ln.startswith("["):
            m = re.match(r"\[(.+?)\]", ln)
            if m:
                tag = m.group(1).lower()
                for key in SECTIONS:
                    if key in tag:
                        section = key
                        break
            continue
        lines.append({"section": section, "text": ln})

    json.dump(lines, open("lines.json", "w"), indent=1)
    print(f"{len(lines)} lyric lines across sections")


if __name__ == "__main__":
    main(sys.argv[1])
