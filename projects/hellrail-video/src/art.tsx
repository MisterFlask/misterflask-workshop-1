import React from 'react';
import {random} from 'remotion';
import {C} from './theme';

/*
 * All art is procedural "pixel" drawing in SVG with crisp edges.
 * Components draw in a local unit grid; `u` = CSS pixels per unit.
 */

const crisp: React.CSSProperties = {shapeRendering: 'crispEdges'};

export const Svg: React.FC<{
  w: number; // units wide
  h: number; // units tall
  u: number;
  style?: React.CSSProperties;
  children: React.ReactNode;
}> = ({w, h, u, style, children}) => (
  <svg
    width={w * u}
    height={h * u}
    viewBox={`0 0 ${w} ${h}`}
    style={{...crisp, overflow: 'visible', ...style}}
  >
    {children}
  </svg>
);

// ---------------------------------------------------------------- string-grid sprites

export const Grid: React.FC<{
  rows: string[];
  pal: Record<string, string>;
}> = ({rows, pal}) => (
  <>
    {rows.map((row, y) =>
      row.split('').map((ch, x) => {
        const fill = pal[ch];
        if (!fill) return null;
        return <rect key={`${x}-${y}`} x={x} y={y} width={1.02} height={1.02} fill={fill} />;
      })
    )}
  </>
);

const IMP_ROWS = [
  '.RR......RR.',
  '..RR....RR..',
  '...RRRRRR...',
  '..RRRRRRRR..',
  '..RYRRRRYR..',
  '..RRRRRRRR..',
  '...RDDDDR...',
  '..RRRRRRRR..',
  '.R.RRRRRR.R.',
  'R..RRRRRR..R',
  '...RRRRRR...',
  '...RR..RR...',
  '..RR....RR..',
  '.RR......RR.',
];
const IMP_PAL = {R: C.demon, Y: C.yellow, D: C.demonDark};

export const Imp: React.FC<{u: number; hop?: number; flip?: boolean}> = ({u, hop = 0, flip}) => (
  <Svg w={12} h={14} u={u} style={{transform: `translateY(${-hop}px) ${flip ? 'scaleX(-1)' : ''}`}}>
    <Grid rows={IMP_ROWS} pal={IMP_PAL} />
  </Svg>
);

const COIN_ROWS = ['.GGGGG.', 'GGYYYGG', 'GYGGGYG', 'GYGYGYG', 'GYGGGYG', 'GGYYYGG', '.GGGGG.'];
export const Coin: React.FC<{u: number; rot?: number}> = ({u, rot = 0}) => (
  <Svg w={7} h={7} u={u} style={{transform: `rotate(${rot}deg)`}}>
    <Grid rows={COIN_ROWS} pal={{G: C.gold, Y: C.goldPale}} />
  </Svg>
);

const SKULL_ROWS = [
  '.WWWWWW.',
  'WWWWWWWW',
  'WBWWWWBW',
  'WBWWWWBW',
  'WWWBBWWW',
  '.WWWWWW.',
  '.WBWBWB.',
  '.WWWWWW.',
];
export const Skull: React.FC<{u: number}> = ({u}) => (
  <Svg w={8} h={8} u={u}>
    <Grid rows={SKULL_ROWS} pal={{W: C.bone, B: C.ink}} />
  </Svg>
);

// ---------------------------------------------------------------- flame

export const Flame: React.FC<{frame: number; seed: string; u: number; hue?: 'orange' | 'blue'}> = ({
  frame,
  seed,
  u,
  hue = 'orange',
}) => {
  const t = Math.floor(frame / 2);
  const f1 = 0.7 + random(`${seed}-a${t}`) * 0.6;
  const f2 = 0.7 + random(`${seed}-b${t}`) * 0.6;
  const outer = hue === 'orange' ? C.ember : '#3060c0';
  const mid = hue === 'orange' ? C.orange : '#60a0e0';
  const core = hue === 'orange' ? C.yellow : C.white;
  return (
    <Svg w={8} h={12} u={u}>
      <rect x={4 - 2.6 * f1} y={12 - 9 * f1} width={5.2 * f1} height={9 * f1} rx={2} fill={outer} />
      <rect x={4 - 1.7 * f2} y={12 - 6.5 * f2} width={3.4 * f2} height={6.5 * f2} rx={1.5} fill={mid} />
      <rect x={4 - 0.9} y={12 - 3.6 * f1} width={1.8} height={3.6 * f1} rx={0.9} fill={core} />
    </Svg>
  );
};

// ---------------------------------------------------------------- THE HELLRAIL TRAIN

export const HellTrain: React.FC<{frame: number; u: number; glow?: number}> = ({frame, u, glow = 0.5}) => {
  const theta = (frame * 16 * Math.PI) / 180;
  const wheel = (cx: number, cy: number, r: number) => (
    <g key={`${cx}`}>
      <circle cx={cx} cy={cy} r={r} fill={C.ink} stroke={C.ash} strokeWidth={0.7} />
      {[0, 1, 2].map((i) => {
        const a = theta + (i * Math.PI) / 3;
        return (
          <line
            key={i}
            x1={cx - Math.cos(a) * r * 0.85}
            y1={cy - Math.sin(a) * r * 0.85}
            x2={cx + Math.cos(a) * r * 0.85}
            y2={cy + Math.sin(a) * r * 0.85}
            stroke={C.ash}
            strokeWidth={0.6}
          />
        );
      })}
      <circle cx={cx} cy={cy} r={r * 0.25} fill={C.brass} />
    </g>
  );
  // crankpin positions for the coupling rod
  const pin = (cx: number) => ({x: cx + Math.cos(theta) * 2.4, y: 23 + Math.sin(theta) * 2.4});
  const p1 = pin(30);
  const p2 = pin(44);
  const glowPulse = 0.35 + glow * 0.55;
  return (
    <Svg w={70} h={30} u={u}>
      {/* under-glow on the rails */}
      <ellipse cx={36} cy={28.5} rx={26} ry={2.5} fill={C.ember} opacity={glowPulse * 0.45} />
      {/* cowcatcher */}
      <polygon points="60,17 68.5,27.5 60,27.5" fill={C.iron} />
      <polygon points="60,19 66.5,27.5 60,27.5" fill={C.blood} />
      {/* boiler */}
      <rect x={22} y={8} width={38} height={12} fill={C.soot} />
      <rect x={22} y={9} width={38} height={1.2} fill={C.ash} opacity={0.6} />
      <rect x={30} y={8} width={1.4} height={12} fill={C.brass} />
      <rect x={38} y={8} width={1.4} height={12} fill={C.brass} />
      <rect x={46} y={8} width={1.4} height={12} fill={C.brass} />
      {/* smokebox face: eye + fanged grin, because the train is ALIVE */}
      <rect x={57.5} y={8} width={2.5} height={12} fill={C.charcoal} />
      <rect x={58} y={10.5} width={1.6} height={2} fill={C.yellow} opacity={0.5 + glow * 0.5} />
      <rect x={58} y={15.5} width={2} height={1} fill={C.blood} />
      <rect x={58.4} y={16.2} width={0.8} height={1.2} fill={C.bone} />
      {/* firebox door glowing on the side */}
      <rect x={24} y={12} width={4.5} height={5.5} fill={C.ember} opacity={glowPulse} />
      <rect x={25} y={13} width={2.5} height={3.5} fill={C.yellow} opacity={glowPulse} />
      {/* smokestack with horns */}
      <rect x={52.5} y={2} width={3.5} height={6.5} fill={C.soot} />
      <rect x={51.8} y={1.2} width={4.9} height={1.6} fill={C.iron} />
      <polygon points="51.8,1.2 50.6,-1.2 52.6,0.6" fill={C.bone} />
      <polygon points="56.7,1.2 57.9,-1.2 55.9,0.6" fill={C.bone} />
      <rect x={53.2} y={0} width={2.1} height={1.2} fill={C.orange} opacity={glowPulse} />
      {/* steam dome */}
      <rect x={40} y={5.4} width={5} height={3} fill={C.brass} />
      <rect x={41} y={4.6} width={3} height={1} fill={C.brass} />
      {/* cab */}
      <rect x={7} y={2.2} width={16} height={1.6} fill={C.iron} />
      <rect x={8} y={3.8} width={14} height={16.2} fill={C.soot} />
      <rect x={10} y={6} width={7} height={6.5} fill={C.orange} opacity={0.6 + glow * 0.4} />
      <rect x={13} y={6} width={1} height={6.5} fill={C.ink} />
      <rect x={10} y={9} width={7} height={0.8} fill={C.ink} />
      {/* gold lettering plate */}
      <rect x={31} y={13.4} width={13} height={2.6} fill={C.ink} stroke={C.gold} strokeWidth={0.35} />
      <text x={37.5} y={15.5} fontSize={2.1} fill={C.gold} textAnchor="middle" fontFamily="Georgia, serif" style={{letterSpacing: 0.3}}>
        HELLRAIL
      </text>
      {/* frame + wheels */}
      <rect x={7} y={20} width={55} height={2.2} fill={C.iron} />
      {wheel(30, 23, 5)}
      {wheel(44, 23, 5)}
      {wheel(14, 25, 3)}
      {wheel(57, 25.5, 2.5)}
      <line x1={p1.x} y1={p1.y} x2={p2.x} y2={p2.y} stroke={C.ash} strokeWidth={1.1} />
    </Svg>
  );
};

export const Carriage: React.FC<{frame: number; u: number; seed: string; glow?: number}> = ({
  frame,
  u,
  seed,
  glow = 0.5,
}) => {
  const theta = (frame * 16 * Math.PI) / 180;
  return (
    <Svg w={34} h={30} u={u}>
      <rect x={1} y={9.4} width={32} height={1.4} fill={C.iron} />
      <rect x={2} y={10.5} width={30} height={9.5} fill={C.purple} />
      <rect x={2} y={10.5} width={30} height={1} fill={C.ash} opacity={0.4} />
      {[0, 1, 2, 3].map((i) => (
        <g key={i}>
          <rect x={4.5 + i * 7} y={12.5} width={4.5} height={4.5} fill={C.orange} opacity={0.55 + glow * 0.45} />
          {random(`${seed}-pass-${i}`) > 0.45 ? (
            <g transform={`translate(${5.6 + i * 7} 13.4) scale(0.28)`}>
              <Grid rows={SKULL_ROWS} pal={{W: C.ink, B: C.orange}} />
            </g>
          ) : null}
        </g>
      ))}
      <rect x={1} y={20} width={32} height={1.8} fill={C.iron} />
      {[8, 26].map((cx) => (
        <g key={cx}>
          <circle cx={cx} cy={24.4} r={2.6} fill={C.ink} stroke={C.ash} strokeWidth={0.6} />
          <line
            x1={cx - Math.cos(theta) * 2}
            y1={24.4 - Math.sin(theta) * 2}
            x2={cx + Math.cos(theta) * 2}
            y2={24.4 + Math.sin(theta) * 2}
            stroke={C.ash}
            strokeWidth={0.5}
          />
        </g>
      ))}
    </Svg>
  );
};

// smoke drifting back from the stack; drawn in screen px
export const SmokeTrail: React.FC<{frame: number; x: number; y: number; scale?: number; n?: number}> = ({
  frame,
  x,
  y,
  scale = 1,
  n = 9,
}) => (
  <svg width={1} height={1} style={{position: 'absolute', left: 0, top: 0, overflow: 'visible'}}>
    {Array.from({length: n}, (_, i) => {
      const age = ((frame * 1.6 + i * (100 / n)) % 100) / 100;
      const drift = random(`smk-${i}`) * 20 - 10;
      return (
        <circle
          key={i}
          cx={x - age * 210 * scale + drift * age}
          cy={y - age * 90 * scale - Math.sin(age * 9 + i) * 8}
          r={(3 + age * 16) * scale}
          fill={age < 0.25 ? C.ash : C.charcoal}
          opacity={(1 - age) * 0.55}
        />
      );
    })}
  </svg>
);

// ---------------------------------------------------------------- DEMONS

export const Demon: React.FC<{
  u: number;
  frame: number;
  variant?: 'banker' | 'barrister' | 'magnate';
  flip?: boolean;
}> = ({u, frame, variant = 'banker', flip}) => {
  const bob = Math.sin(frame / 9) * 0.5;
  return (
    <Svg w={18} h={28} u={u} style={{transform: flip ? 'scaleX(-1)' : undefined}}>
      <g transform={`translate(0 ${bob})`}>
        {/* horns */}
        <polygon points="4.2,6 3,1.5 5.6,4.5" fill={C.bone} />
        <polygon points="13.8,6 15,1.5 12.4,4.5" fill={C.bone} />
        {variant === 'barrister' ? (
          <g>
            {/* horsehair wig */}
            <rect x={4} y={3} width={10} height={3.4} rx={1.4} fill={C.white} />
            <rect x={2.8} y={5} width={2.4} height={5.5} rx={1.2} fill={C.white} />
            <rect x={12.8} y={5} width={2.4} height={5.5} rx={1.2} fill={C.white} />
            <rect x={4.5} y={4.2} width={9} height={0.6} fill={C.parchmentDark} opacity={0.5} />
          </g>
        ) : (
          <g>
            {/* top hat */}
            <rect x={2.5} y={5.6} width={13} height={1.4} fill={C.ink} />
            <rect x={4.6} y={0.2} width={8.8} height={5.6} fill={C.ink} />
            <rect x={4.6} y={4.2} width={8.8} height={1.2} fill={variant === 'magnate' ? C.gold : C.blood} />
          </g>
        )}
        {/* face */}
        <rect x={5} y={7} width={8} height={6} fill={C.demon} />
        <rect x={6.2} y={8.6} width={1.6} height={1.6} fill={C.yellow} />
        <rect x={10.2} y={8.6} width={1.6} height={1.6} fill={C.yellow} />
        {variant === 'magnate' ? (
          <circle cx={11} cy={9.4} r={1.6} fill="none" stroke={C.gold} strokeWidth={0.5} />
        ) : null}
        <rect x={6.4} y={11.6} width={5.2} height={0.9} fill={C.demonDark} />
        <rect x={7} y={11.6} width={0.9} height={1.5} fill={C.bone} />
        <rect x={10} y={11.6} width={0.9} height={1.5} fill={C.bone} />
        {/* cigar */}
        {variant === 'banker' ? (
          <g>
            <rect x={12.4} y={11.4} width={3.4} height={1} fill="#6a4020" />
            <rect x={15.4} y={11.4} width={0.9} height={1} fill={C.orange} opacity={0.6 + 0.4 * Math.sin(frame / 5)} />
          </g>
        ) : null}
        {/* coat */}
        <rect x={4} y={13} width={10} height={11} fill={C.ink} />
        <polygon points="4,13 8,13 5.5,17" fill={C.charcoal} />
        <polygon points="14,13 10,13 12.5,17" fill={C.charcoal} />
        <rect x={8} y={13.2} width={2} height={4.5} fill={C.white} />
        <rect x={8.4} y={13.6} width={1.2} height={1.2} fill={variant === 'magnate' ? C.gold : C.blood} />
        {/* watch chain */}
        <rect x={5.6} y={17.5} width={0.7} height={0.7} fill={C.gold} />
        <rect x={6.4} y={18.3} width={0.7} height={0.7} fill={C.gold} />
        <rect x={7.2} y={18.8} width={0.7} height={0.7} fill={C.gold} />
        {/* arms */}
        <rect x={2.4} y={13.6} width={2} height={7.5} fill={C.ink} />
        <rect x={13.6} y={13.6} width={2} height={7.5} fill={C.ink} />
        <rect x={2.4} y={20.8} width={2} height={1.6} fill={C.demon} />
        <rect x={13.6} y={20.8} width={2} height={1.6} fill={C.demon} />
        {/* held item */}
        {variant === 'barrister' ? (
          <rect x={0.2} y={21} width={5} height={2.2} rx={1.1} fill={C.parchment} />
        ) : (
          <g>
            <rect x={0.2} y={21.6} width={5.4} height={4} rx={0.5} fill="#5a3818" />
            <rect x={2.4} y={21.2} width={1} height={1} fill={C.gold} />
          </g>
        )}
        {/* legs */}
        <rect x={5.4} y={24} width={2.8} height={3.6} fill={C.ink} />
        <rect x={9.8} y={24} width={2.8} height={3.6} fill={C.ink} />
        {/* tail */}
        <rect x={2.6} y={24.5} width={2} height={0.9} fill={C.demon} />
        <rect x={1.6} y={23} width={1} height={2} fill={C.demon} />
        <polygon points="2.1,23 0.6,21.4 3.2,21.9" fill={C.demon} />
      </g>
    </Svg>
  );
};

export const Widow: React.FC<{u: number; frame: number}> = ({u, frame}) => {
  const bob = Math.sin(frame / 6) * 0.4;
  const wobble = Math.sin(frame / 6 + 2) * 6;
  return (
    <Svg w={34} h={28} u={u}>
      <g transform={`translate(0 ${bob})`}>
        {/* bonnet + face */}
        <rect x={3.4} y={1.6} width={7.2} height={4.4} rx={1.6} fill={C.ink} />
        <rect x={4.2} y={5.4} width={5.6} height={0.9} fill={C.ash} />
        <rect x={4.6} y={6} width={4.8} height={3.6} fill="#c8b8a8" />
        <rect x={5.6} y={7} width={1} height={1} fill={C.ink} />
        <rect x={7.6} y={7} width={1} height={1} fill={C.ink} />
        <rect x={5.8} y={8.7} width={2.6} height={0.5} fill="#907868" />
        {/* dress */}
        <rect x={4} y={9.6} width={6} height={4} fill={C.ink} />
        <rect x={3} y={13} width={8} height={6} fill={C.ink} />
        <rect x={2} y={18} width={10} height={8} fill={C.ink} />
        <rect x={2} y={25} width={10} height={1} fill={C.charcoal} />
        {/* arms pushing */}
        <rect x={9} y={11.4} width={6.5} height={1.6} fill={C.ink} />
        <rect x={15} y={11.4} width={1.4} height={1.4} fill="#c8b8a8" />
        {/* barrow */}
        <g transform={`rotate(${wobble * 0.06} 22 20)`}>
          <polygon points="15,13.5 31,13.5 28.5,20 17,20" fill="#5a3818" />
          <rect x={15} y={12.8} width={16} height={1} fill="#6a4828" />
          {[17, 19.5, 22, 24.5, 27].map((x, i) => (
            <g key={i} transform={`translate(${x} ${10.6 + (i % 2) * 0.8}) scale(0.34)`}>
              <Grid rows={COIN_ROWS} pal={{G: C.gold, Y: C.goldPale}} />
            </g>
          ))}
          <circle cx={20} cy={23} r={2.8} fill={C.ink} stroke={C.ash} strokeWidth={0.5} />
          <line x1={20 - Math.cos(frame / 4) * 2.2} y1={23 - Math.sin(frame / 4) * 2.2} x2={20 + Math.cos(frame / 4) * 2.2} y2={23 + Math.sin(frame / 4) * 2.2} stroke={C.ash} strokeWidth={0.5} />
          <rect x={26} y={20} width={1.2} height={5} fill="#5a3818" />
        </g>
      </g>
    </Svg>
  );
};

export const MatchSeller: React.FC<{u: number; frame: number}> = ({u, frame}) => (
  <Svg w={18} h={30} u={u}>
    {/* messy hair + gaunt face */}
    <rect x={5.6} y={2} width={6.8} height={2.4} fill={C.ash} />
    <rect x={5} y={3} width={1.4} height={2.4} fill={C.ash} />
    <rect x={6} y={4.4} width={6} height={4.4} fill="#b8a890" />
    <rect x={7.2} y={5.8} width={1} height={1.2} fill={C.ink} />
    <rect x={9.8} y={5.8} width={1} height={1.2} fill={C.ink} />
    <rect x={7.6} y={7.9} width={2.8} height={0.5} fill="#807058" />
    {/* ragged coat with patches */}
    <rect x={4.4} y={8.8} width={9.2} height={12} fill={C.charcoal} />
    <rect x={5.4} y={11} width={2.2} height={2.2} fill={C.ash} opacity={0.7} />
    <rect x={10} y={15} width={2.4} height={2.4} fill={C.rainBlue} opacity={0.7} />
    <polygon points="4.4,20.8 6.4,20.8 5.4,23 4.4,23" fill={C.charcoal} />
    <polygon points="13.6,20.8 11.6,20.8 12.6,22 13.6,22" fill={C.charcoal} />
    {/* arm raised holding a match */}
    <rect x={12.2} y={9.6} width={1.8} height={6} fill={C.charcoal} transform="rotate(-28 13 12)" />
    <rect x={14.4} y={7.4} width={1.5} height={1.5} fill="#b8a890" />
    <rect x={15} y={4.4} width={0.7} height={3.4} fill={C.bone} />
    {/* matchbox tray */}
    <rect x={5} y={14.6} width={7} height={2.6} fill="#5a3818" />
    {[5.8, 7.2, 8.6, 10].map((x) => (
      <rect key={x} x={x} y={13.9} width={0.7} height={1} fill={C.bone} />
    ))}
    {/* legs */}
    <rect x={6} y={20.8} width={2.4} height={5.8} fill={C.iron} />
    <rect x={9.6} y={20.8} width={2.4} height={5.8} fill={C.iron} />
    <rect x={5.6} y={26.6} width={3.2} height={1.2} fill={C.ink} />
    <rect x={9.2} y={26.6} width={3.2} height={1.2} fill={C.ink} />
  </Svg>
);

// ---------------------------------------------------------------- ENVIRONMENT

export const Skyline: React.FC<{w: number; h: number; color?: string; seed?: string}> = ({
  w,
  h,
  color = C.ink,
  seed = 'sky',
}) => {
  const buildings: React.ReactNode[] = [];
  let x = 0;
  let i = 0;
  while (x < w) {
    const bw = 30 + random(`${seed}-w${i}`) * 60;
    const bh = h * (0.25 + random(`${seed}-h${i}`) * 0.55);
    buildings.push(<rect key={`b${i}`} x={x} y={h - bh} width={bw} height={bh} fill={color} />);
    if (random(`${seed}-c${i}`) > 0.4) {
      buildings.push(
        <rect key={`c${i}`} x={x + bw * 0.2} y={h - bh - 12} width={6} height={12} fill={color} />
      );
    }
    x += bw + 2 + random(`${seed}-g${i}`) * 8;
    i++;
  }
  const dx = w * 0.42; // St Paul's dome
  const bx = w * 0.78; // clock tower
  return (
    <svg width={w} height={h} style={{...crisp, position: 'absolute', bottom: 0, left: 0, overflow: 'visible'}}>
      {buildings}
      <rect x={dx - 40} y={h * 0.52} width={80} height={h * 0.48} fill={color} />
      <path d={`M ${dx - 34} ${h * 0.52} Q ${dx} ${h * 0.1} ${dx + 34} ${h * 0.52} Z`} fill={color} />
      <rect x={dx - 3} y={h * 0.1 - 16} width={6} height={20} fill={color} />
      <rect x={bx - 12} y={h * 0.28} width={24} height={h * 0.72} fill={color} />
      <polygon points={`${bx - 12},${h * 0.28} ${bx},${h * 0.16} ${bx + 12},${h * 0.28}`} fill={color} />
      <circle cx={bx} cy={h * 0.36} r={7} fill={C.gold} opacity={0.9} />
      <circle cx={bx} cy={h * 0.36} r={7} fill="none" stroke={color} strokeWidth={2.5} />
      <line x1={bx} y1={h * 0.36} x2={bx} y2={h * 0.36 - 5} stroke={color} strokeWidth={1.5} />
      <line x1={bx} y1={h * 0.36} x2={bx + 4} y2={h * 0.36} stroke={color} strokeWidth={1.5} />
    </svg>
  );
};

export const LavaSea: React.FC<{frame: number; w: number; h: number; top: number}> = ({frame, w, h, top}) => {
  const rows = [0, 1, 2, 3];
  return (
    <svg width={w} height={h} style={{...crisp, position: 'absolute', top, left: 0}}>
      <rect x={0} y={0} width={w} height={h} fill={C.blood} />
      <rect x={0} y={0} width={w} height={h * 0.3} fill={C.red} />
      {rows.map((r) =>
        Array.from({length: 14}, (_, i) => {
          const seg = w / 13;
          const xo = ((frame * (1.2 + r * 0.5) + r * 40) % (seg * 2)) - seg;
          const y = h * 0.12 + r * (h * 0.22) + Math.sin(frame / 14 + i * 2 + r) * 3;
          return (
            <rect
              key={`${r}-${i}`}
              x={i * seg - xo}
              y={y}
              width={seg * 0.55}
              height={4}
              fill={r % 2 ? C.ember : C.orange}
              opacity={0.8}
            />
          );
        })
      )}
      {Array.from({length: 5}, (_, i) => {
        const cyc = Math.floor(frame / 26) + i;
        const bx = random(`bub-${cyc}-${i}`) * w;
        const bt = ((frame + i * 9) % 26) / 26;
        return <circle key={i} cx={bx} cy={h * 0.3} r={bt * 8} fill={C.flame} opacity={(1 - bt) * 0.7} />;
      })}
    </svg>
  );
};

export const Crags: React.FC<{w: number; h: number; seed: string; color: string; y: number; drift?: number}> = ({
  w,
  h,
  seed,
  color,
  y,
  drift = 0,
}) => {
  const n = 18;
  const pts: string[] = [`${-drift % (w / 4)},${h}`];
  for (let i = 0; i <= n; i++) {
    const px = (i / n) * (w * 1.3) - (drift % (w / 4));
    const py = h - random(`${seed}-${i}`) * h * 0.9;
    pts.push(`${px},${py}`);
  }
  pts.push(`${w * 1.3},${h}`);
  return (
    <svg width={w} height={h} style={{...crisp, position: 'absolute', top: y, left: 0}}>
      <polygon points={pts.join(' ')} fill={color} />
    </svg>
  );
};

export const RainLayer: React.FC<{frame: number; w: number; h: number; n?: number; color?: string}> = ({
  frame,
  w,
  h,
  n = 70,
  color = C.grayBlue,
}) => (
  <svg width={w} height={h} style={{position: 'absolute', inset: 0}}>
    {Array.from({length: n}, (_, i) => {
      const x = (random(`rx-${i}`) * w + frame * 1.2) % w;
      const y = (random(`ry-${i}`) * h + frame * (14 + random(`rv-${i}`) * 10)) % h;
      const len = 10 + random(`rl-${i}`) * 12;
      return (
        <line key={i} x1={x} y1={y} x2={x - 3} y2={y + len} stroke={color} strokeWidth={1.4} opacity={0.4} />
      );
    })}
  </svg>
);

export const FireworkBurst: React.FC<{t: number; x: number; y: number; color: string; size?: number}> = ({
  t,
  x,
  y,
  color,
  size = 70,
}) => {
  if (t <= 0 || t >= 1) return null;
  const r = size * Math.min(1, t * 1.6);
  return (
    <svg width={1} height={1} style={{position: 'absolute', left: 0, top: 0, overflow: 'visible'}}>
      {Array.from({length: 10}, (_, i) => {
        const a = (i / 10) * Math.PI * 2;
        return (
          <line
            key={i}
            x1={x + Math.cos(a) * r * 0.4}
            y1={y + Math.sin(a) * r * 0.4 + t * t * 20}
            x2={x + Math.cos(a) * r}
            y2={y + Math.sin(a) * r + t * t * 20}
            stroke={color}
            strokeWidth={3 * (1 - t)}
            opacity={1 - t}
          />
        );
      })}
      {t < 0.3 ? <circle cx={x} cy={y} r={6 * (1 - t / 0.3)} fill={C.white} /> : null}
    </svg>
  );
};

export const Gavel: React.FC<{u: number; angle: number}> = ({u, angle}) => (
  <Svg w={26} h={20} u={u}>
    <g transform={`rotate(${angle} 3 17)`}>
      <rect x={2} y={15.6} width={17} height={2.4} rx={1} fill="#6a4828" />
      <rect x={16.5} y={9.5} width={7.5} height={13} rx={1.4} fill="#4a2c14" />
      <rect x={16.5} y={10.6} width={7.5} height={1.6} fill={C.brass} />
      <rect x={16.5} y={19.4} width={7.5} height={1.6} fill={C.brass} />
    </g>
    <rect x={0} y={19} width={10} height={1.4} fill={C.iron} />
  </Svg>
);

// stock chart that climbs, then dies
export const StockChart: React.FC<{
  w: number;
  h: number;
  progress: number; // 0..1 how much of the line is drawn
  crashAt?: number; // progress fraction where it turns into a cliff (default: never)
  color?: string;
}> = ({w, h, progress, crashAt = 2, color = C.gold}) => {
  const n = 60;
  const pts: Array<[number, number]> = [];
  const upTo = Math.floor(n * Math.min(1, progress));
  for (let i = 0; i <= upTo; i++) {
    const f = i / n;
    const x = f * w;
    let y: number;
    if (f < crashAt) {
      y = h * 0.9 - f * h * 0.75 + (random(`chart-${i}`) - 0.5) * h * 0.14;
    } else {
      const cf = (f - crashAt) / Math.max(0.001, 1 - crashAt);
      const deadCat = Math.max(0, Math.sin(cf * 9)) * 0.08 * (1 - cf);
      y = h * (0.9 - 0.75 * crashAt) + cf * h * 1.35 - deadCat * h + (random(`chart-${i}`) - 0.5) * h * 0.06;
    }
    pts.push([x, Math.min(h * 1.4, y)]);
  }
  const head = pts[pts.length - 1];
  return (
    <svg width={w} height={h} style={{position: 'absolute', overflow: 'hidden'}}>
      {[0.25, 0.5, 0.75].map((g) => (
        <line key={g} x1={0} y1={h * g} x2={w} y2={h * g} stroke={C.ash} strokeWidth={1} opacity={0.25} />
      ))}
      <polyline
        points={pts.map(([x, y]) => `${x},${y}`).join(' ')}
        fill="none"
        stroke={color}
        strokeWidth={3.5}
        strokeLinejoin="round"
      />
      {head ? <circle cx={head[0]} cy={head[1]} r={5} fill={C.white} /> : null}
    </svg>
  );
};

export const PieChart: React.FC<{size: number; fraction: number}> = ({size, fraction}) => {
  const r = size / 2 - 6;
  const cx = size / 2;
  const cy = size / 2;
  const a = Math.max(0.003, fraction) * Math.PI * 2;
  const x = cx + Math.sin(a) * r;
  const y = cy - Math.cos(a) * r;
  const large = a > Math.PI ? 1 : 0;
  return (
    <svg width={size} height={size} style={{overflow: 'visible'}}>
      <circle cx={cx} cy={cy} r={r} fill={C.demonDark} stroke={C.bone} strokeWidth={2} />
      <path d={`M ${cx} ${cy} L ${cx} ${cy - r} A ${r} ${r} 0 ${large} 1 ${x} ${y} Z`} fill={C.gold} />
    </svg>
  );
};

export const PentagramSeal: React.FC<{size: number; color?: string; spin?: number}> = ({
  size,
  color = C.red,
  spin = 0,
}) => {
  const r = size / 2 - 4;
  const cx = size / 2;
  const cy = size / 2;
  const pt = (i: number) => {
    const a = (i * 4 * Math.PI) / 5 - Math.PI / 2 + (spin * Math.PI) / 180;
    return `${cx + Math.cos(a) * r},${cy + Math.sin(a) * r}`;
  };
  return (
    <svg width={size} height={size} style={{overflow: 'visible'}}>
      <circle cx={cx} cy={cy} r={r} fill="none" stroke={color} strokeWidth={size * 0.03} />
      <polygon
        points={[0, 1, 2, 3, 4].map(pt).join(' ')}
        fill="none"
        stroke={color}
        strokeWidth={size * 0.03}
        strokeLinejoin="miter"
      />
    </svg>
  );
};
