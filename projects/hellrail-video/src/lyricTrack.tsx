import React from 'react';
import {AbsoluteFill, Sequence, interpolate, random, useCurrentFrame} from 'remotion';
import lyricsData from './lyrics.json';
import {C, SERIF} from './theme';

export type LyricWord = {w: string; f: number};
export type LyricLine = {text: string; stanza: number; start: number; end: number; words: LyricWord[]};
export const LYRICS = lyricsData as {lines: LyricLine[]; stanzas: Array<{start: number; end: number}>};

/** Per-stanza card placement/styling so cards sit well in each scene. */
type CardStyle = {y: string; size: number; color: string};
const STANZA_STYLE: Record<number, CardStyle> = {
  0: {y: '80%', size: 36, color: C.bone}, // blueprint
  1: {y: '80%', size: 34, color: C.bone}, // boiler
  2: {y: '78%', size: 34, color: C.bone}, // wheels
  3: {y: '78%', size: 34, color: C.bone}, // descent (overridden below)
  4: {y: '80%', size: 34, color: C.bone}, // luxury
  5: {y: '80%', size: 34, color: C.bone}, // velocity
  6: {y: '24%', size: 34, color: C.bone}, // fuel (furnace mouth owns the bottom)
  7: {y: '76%', size: 34, color: C.goldPale}, // finale
};
/** Stanzas whose scenes place lyrics high (gauges at top instead) */
export const DESCENT_STANZA_OVERRIDE: CardStyle = {y: '30%', size: 38, color: C.bone};

const LEAD_IN = 5;
const TAIL = 14;

export const LyricTrack: React.FC<{descentRange?: [number, number]}> = ({descentRange}) => {
  return (
    <AbsoluteFill style={{pointerEvents: 'none'}}>
      {LYRICS.lines.map((line, i) => {
        const next = LYRICS.lines[i + 1];
        const from = Math.max(0, line.start - LEAD_IN);
        const rawEnd = line.end + TAIL;
        const end = next ? Math.min(rawEnd, next.start - LEAD_IN - 1) : rawEnd;
        const dur = Math.max(20, end - from);
        const inDescent = descentRange && line.start >= descentRange[0] && line.start < descentRange[1];
        const style = inDescent ? DESCENT_STANZA_OVERRIDE : STANZA_STYLE[line.stanza] ?? STANZA_STYLE[0];
        return (
          <Sequence key={i} from={from} durationInFrames={dur}>
            <KaraokeCard line={line} absFrom={from} dur={dur} style={style} />
          </Sequence>
        );
      })}
    </AbsoluteFill>
  );
};

const KaraokeCard: React.FC<{line: LyricLine; absFrom: number; dur: number; style: CardStyle}> = ({
  line,
  absFrom,
  dur,
  style,
}) => {
  const f = useCurrentFrame();
  const abs = absFrom + f;
  const inS = interpolate(f, [0, 5], [0, 1], {extrapolateRight: 'clamp'});
  const out = interpolate(f, [dur - 6, dur - 1], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const rot = (random(line.text) - 0.5) * 3;
  return (
    <AbsoluteFill style={{alignItems: 'center', justifyContent: 'flex-start'}}>
      <div
        style={{
          position: 'absolute',
          top: style.y,
          transform: `translateY(-50%) rotate(${rot}deg) scale(${0.9 + inS * 0.1})`,
          opacity: inS * out,
          background: 'rgba(10,5,5,0.84)',
          border: `3px solid ${C.brass}`,
          boxShadow: '0 0 0 6px rgba(10,5,5,0.84), 0 0 40px rgba(0,0,0,0.6)',
          padding: '12px 34px',
          maxWidth: '88%',
          textAlign: 'center',
          fontFamily: SERIF,
          fontWeight: 700,
          fontSize: style.size,
          lineHeight: 1.2,
          letterSpacing: 1.5,
        }}
      >
        {line.words.map((word, wi) => {
          const lit = abs >= word.f;
          const sincePop = abs - word.f;
          const pop = lit && sincePop < 4 ? 1 + (1 - sincePop / 4) * 0.1 : 1;
          return (
            <React.Fragment key={wi}>
              <span
                style={{
                  display: 'inline-block',
                  color: lit ? style.color : 'rgba(232,220,192,0.3)',
                  textShadow: lit ? `0 3px 0 ${C.ink}, 0 0 22px rgba(232,88,24,0.5)` : 'none',
                  transform: `scale(${pop})`,
                }}
              >
                {word.w.toUpperCase()}
              </span>
              {wi < line.words.length - 1 ? ' ' : ''}
            </React.Fragment>
          );
        })}
      </div>
    </AbsoluteFill>
  );
};
