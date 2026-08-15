#!/usr/bin/env python3
"""Build a styled ASS karaoke subtitle track from timed_lines.json.

Section-aware styling: warm white verses, molten pop-in choruses, icy blue
serif whispers for the bridge (hell freezing over), oversized break line,
plus title and end cards.

Usage: make_subtitles.py   (reads timed_lines.json, writes lyrics.ass)
"""
import json


def ts(t):
    t = max(0.0, t)
    return f"{int(t // 3600)}:{int(t % 3600 // 60):02d}:{t % 60:05.2f}"


HEADER = """[Script Info]
Title: Stokers of the Underworld
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080
WrapStyle: 0
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Verse,DejaVu Sans,62,&H00D8E8FF,&H00FFFFFF,&H00101060,&H80000000,-1,0,0,0,100,100,1,0,1,4,2,5,120,120,10,1
Style: Chorus,DejaVu Sans,76,&H002CC8FF,&H00FFFFFF,&H000A0A78,&H80000000,-1,0,0,0,100,100,2,0,1,5,3,5,90,90,10,1
Style: Bridge,DejaVu Serif,54,&H00FFE2B8,&H00FFFFFF,&H00583010,&H80000000,0,-1,0,0,100,100,1,0,1,3,2,5,150,150,10,1
Style: Break,DejaVu Sans,84,&H00FFFFFF,&H00FFFFFF,&H000000A0,&H80000000,-1,0,0,0,100,100,2,0,1,6,4,5,80,80,10,1
Style: Intro,DejaVu Sans,66,&H00E8E8E8,&H00FFFFFF,&H00202020,&H80000000,-1,0,0,0,100,100,1,0,1,4,3,5,120,120,10,1
Style: Title,DejaVu Serif,92,&H001A38F0,&H00FFFFFF,&H00050518,&H80000000,-1,0,0,0,100,100,3,0,1,5,4,5,80,80,10,1
Style: Sub,DejaVu Sans,44,&H00B8C8E8,&H00FFFFFF,&H00101040,&H80000000,0,0,0,0,100,100,4,0,1,2,2,5,80,80,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"""

POS = r"\an5\pos(960,600)"
STYLE_MAP = {"intro": "Intro", "verse": "Verse", "chorus": "Chorus",
             "bridge": "Bridge", "break": "Break", "final chorus": "Chorus"}
FX = {
    "Chorus": r"{" + POS + r"\fad(150,200)\fscx82\fscy82\t(0,130,\fscx100\fscy100)}",
    "Break": r"{" + POS + r"\fad(100,300)\fscx70\fscy70\t(0,160,\fscx104\fscy104)\t(160,300,\fscx100\fscy100)}",
    "Bridge": r"{" + POS + r"\fad(600,700)}",
    "Verse": r"{" + POS + r"\fad(200,250)}",
    "Intro": r"{\an5\pos(960,720)\fad(200,250)}",  # keeps clear of the title card
}


def main():
    lines = json.load(open("timed_lines.json"))
    events = []

    def ev(start, end, style, text):
        events.append(f"Dialogue: 0,{ts(start)},{ts(end)},{style},,0,0,0,,{text}")

    ev(0.0, 6.4, "Title", r"{\an5\pos(960,300)\fad(400,400)}STOKERS OF THE\NUNDERWORLD")
    ev(0.0, 6.4, "Sub", r"{\an5\pos(960,470)\fad(700,400)}⚒  ironlordbyron  ⚒")

    for i, l in enumerate(lines):
        style = STYLE_MAP[l["section"]]
        dstart = max(0.0, l["start"] - 0.35)
        dend = l["end"] + 0.7
        if i + 1 < len(lines):
            dend = max(min(dend, lines[i + 1]["start"] - 0.30), l["end"] + 0.1)
        fx = FX["Intro"] if l["section"] == "intro" else FX[style]
        ev(dstart, dend, style, fx + l["text"])

    ev(119.0, 127.2, "Title", r"{\an5\pos(960,430)\fad(600,800)}⚒ UNITE! ⚒")
    ev(119.6, 127.2, "Sub", r"{\an5\pos(960,580)\fad(800,800)}Stokers of the Underworld · made with Suno")

    open("lyrics.ass", "w").write(HEADER + "\n".join(events) + "\n")
    print(f"{len(events)} subtitle events")


if __name__ == "__main__":
    main()
