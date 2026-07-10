import React from 'react';
import {AbsoluteFill, Sequence, interpolate, random, useCurrentFrame} from 'remotion';
import {C, SERIF} from './theme';
import {punch, transientAt} from './audio';

/** Victorian poster-style lyric card. Mount inside a Sequence. */
export const LyricCard: React.FC<{
  text: string;
  from: number;
  dur: number;
  size?: number;
  color?: string;
  bg?: boolean;
  y?: string; // css top
  tilt?: number;
}> = ({text, from, dur, size = 46, color = C.bone, bg = true, y = '72%', tilt}) => {
  return (
    <Sequence from={from} durationInFrames={dur}>
      <LyricCardInner text={text} dur={dur} size={size} color={color} bg={bg} y={y} tilt={tilt} />
    </Sequence>
  );
};

const LyricCardInner: React.FC<{
  text: string;
  dur: number;
  size: number;
  color: string;
  bg: boolean;
  y: string;
  tilt?: number;
}> = ({text, dur, size, color, bg, y, tilt}) => {
  const f = useCurrentFrame();
  const inS = interpolate(f, [0, 7], [0, 1], {extrapolateRight: 'clamp'});
  const pop = 1 + Math.max(0, 1 - f / 7) * 0.25;
  const out = interpolate(f, [dur - 8, dur - 1], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const rot = tilt ?? (random(text) - 0.5) * 4;
  return (
    <AbsoluteFill style={{alignItems: 'center', justifyContent: 'flex-start', pointerEvents: 'none'}}>
      <div
        style={{
          position: 'absolute',
          top: y,
          transform: `translateY(-50%) rotate(${rot}deg) scale(${pop * inS})`,
          opacity: inS * out,
          background: bg ? 'rgba(10,5,5,0.82)' : 'none',
          border: bg ? `3px solid ${C.brass}` : 'none',
          boxShadow: bg ? `0 0 0 6px rgba(10,5,5,0.82), 0 0 40px rgba(0,0,0,0.6)` : 'none',
          padding: bg ? '14px 38px' : 0,
          maxWidth: '86%',
          textAlign: 'center',
          fontFamily: SERIF,
          fontWeight: 700,
          fontSize: size,
          lineHeight: 1.15,
          letterSpacing: 2,
          color,
          textShadow: `0 3px 0 ${C.ink}, 0 0 26px rgba(232,88,24,0.35)`,
        }}
      >
        {text}
      </div>
    </AbsoluteFill>
  );
};

/** Huge single word slammed on screen for a few frames. */
export const StampWord: React.FC<{
  word: string;
  from: number;
  dur?: number;
  color?: string;
  size?: number;
  rot?: number;
}> = ({word, from, dur = 12, color = C.red, size = 170, rot}) => (
  <Sequence from={from} durationInFrames={dur}>
    <StampInner word={word} dur={dur} color={color} size={size} rot={rot} />
  </Sequence>
);

const StampInner: React.FC<{word: string; dur: number; color: string; size: number; rot?: number}> = ({
  word,
  dur,
  color,
  size,
  rot,
}) => {
  const f = useCurrentFrame();
  const s = interpolate(f, [0, 3], [2.6, 1], {extrapolateRight: 'clamp'});
  const o = interpolate(f, [dur - 4, dur - 1], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const r = rot ?? (random(word) - 0.5) * 18;
  return (
    <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center'}}>
      <div
        style={{
          transform: `rotate(${r}deg) scale(${s})`,
          opacity: o,
          fontFamily: SERIF,
          fontWeight: 900,
          fontSize: size,
          letterSpacing: 8,
          color,
          border: `10px double ${color}`,
          padding: '4px 44px',
          textShadow: '0 0 30px rgba(0,0,0,0.8)',
          background: 'rgba(10,5,5,0.35)',
        }}
      >
        {word}
      </div>
    </AbsoluteFill>
  );
};

/** Global CRT-ish patina: scanlines + vignette + film specks. */
export const Patina: React.FC<{frame: number}> = ({frame}) => {
  const specks = Array.from({length: 14}, (_, i) => {
    const cyc = Math.floor(frame / 4);
    return {
      x: random(`spk-${cyc}-${i}`) * 100,
      y: random(`spk2-${cyc}-${i}`) * 100,
      w: 1 + random(`spk3-${cyc}-${i}`) * 2,
    };
  });
  return (
    <AbsoluteFill style={{pointerEvents: 'none'}}>
      <AbsoluteFill
        style={{
          background:
            'repeating-linear-gradient(0deg, rgba(0,0,0,0.16) 0px, rgba(0,0,0,0.16) 1px, transparent 1px, transparent 4px)',
        }}
      />
      <AbsoluteFill
        style={{
          background: 'radial-gradient(ellipse at center, transparent 52%, rgba(0,0,0,0.55) 100%)',
        }}
      />
      <svg width="100%" height="100%" style={{position: 'absolute', opacity: 0.5}}>
        {specks.map((s, i) => (
          <rect key={i} x={`${s.x}%`} y={`${s.y}%`} width={s.w} height={s.w * 2.5} fill={C.bone} opacity={0.35} />
        ))}
      </svg>
    </AbsoluteFill>
  );
};

const FLASH_WORDS = [
  'BUY!', 'DIG!', 'PROFIT!', 'SIGN!', 'BOARD!', 'SHARES!', 'GOLD!', 'BRAVO!',
  'SPECULATE!', 'COMPOUND!', 'GUARANTEED*', 'HELLWARD!', 'INVEST!', 'DIVIDENDS!',
  'SULPHUR!', 'PROGRESS!', 'STEAM!', 'MAMMON!', 'HOTTER!', 'DEEPER!', 'FASTER!',
];

/** Beat-synced strobe overlay: every transient jolts; every 3rd flashes a word. */
export const BeatFlashes: React.FC<{frame: number}> = ({frame}) => {
  const idx = transientAt(frame);
  if (idx < 0) return null;
  const wordy = idx % 3 === 0;
  if (!wordy) {
    return <AbsoluteFill style={{background: C.white, opacity: 0.12}} />;
  }
  const word = FLASH_WORDS[(idx / 3) % FLASH_WORDS.length | 0];
  const invert = idx % 6 === 0;
  return (
    <AbsoluteFill
      style={{
        background: invert ? C.bone : 'rgba(138,16,16,0.85)',
        alignItems: 'center',
        justifyContent: 'center',
      }}
    >
      <div
        style={{
          fontFamily: SERIF,
          fontWeight: 900,
          fontSize: 180,
          letterSpacing: 10,
          color: invert ? C.blood : C.bone,
          transform: `rotate(${(random(`fw-${idx}`) - 0.5) * 10}deg) scale(${1 + (frame % 3) * 0.06})`,
        }}
      >
        {word}
      </div>
    </AbsoluteFill>
  );
};

/** Camera wrapper: audio-reactive zoom pulse + shake. */
export const Camera: React.FC<{
  frame: number;
  shake?: number; // extra shake multiplier
  children: React.ReactNode;
}> = ({frame, shake = 1, children}) => {
  const p = punch(frame);
  const jx = (random(`shx-${frame}`) - 0.5) * p * 46 * shake;
  const jy = (random(`shy-${frame}`) - 0.5) * p * 46 * shake;
  const rot = (random(`shr-${frame}`) - 0.5) * p * 2.4 * shake;
  const zoom = 1 + p * 0.06 * shake;
  return (
    <AbsoluteFill
      style={{
        transform: `translate(${jx}px, ${jy}px) rotate(${rot}deg) scale(${zoom})`,
      }}
    >
      {children}
    </AbsoluteFill>
  );
};

/** Cheap chromatic-aberration-ish double exposure for the crash. */
export const Aberrate: React.FC<{amount: number; children: React.ReactNode}> = ({amount, children}) => (
  <AbsoluteFill>
    <AbsoluteFill style={{filter: `drop-shadow(${amount}px 0 0 rgba(196,36,24,0.5)) drop-shadow(${-amount}px 0 0 rgba(48,96,192,0.4))`}}>
      {children}
    </AbsoluteFill>
  </AbsoluteFill>
);

/** Ornate double border frame for poster scenes. */
export const PosterFrame: React.FC<{color?: string; inset?: number}> = ({color = C.brass, inset = 26}) => (
  <AbsoluteFill style={{pointerEvents: 'none'}}>
    <div
      style={{
        position: 'absolute',
        inset,
        border: `3px solid ${color}`,
        boxShadow: `inset 0 0 0 6px transparent`,
      }}
    />
    <div style={{position: 'absolute', inset: inset + 9, border: `1.5px solid ${color}`, opacity: 0.7}} />
    {[
      {top: inset - 7, left: inset - 7},
      {top: inset - 7, right: inset - 7},
      {bottom: inset - 7, left: inset - 7},
      {bottom: inset - 7, right: inset - 7},
    ].map((pos, i) => (
      <div
        key={i}
        style={{
          position: 'absolute',
          ...pos,
          width: 15,
          height: 15,
          background: color,
          transform: 'rotate(45deg)',
        }}
      />
    ))}
  </AbsoluteFill>
);
