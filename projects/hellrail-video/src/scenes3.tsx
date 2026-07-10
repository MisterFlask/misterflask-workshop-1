import React from 'react';
import {AbsoluteFill, interpolate, random, useCurrentFrame} from 'remotion';
import {C, SERIF} from './theme';
import {energy, punch} from './audio';
import {
  Coin,
  Crags,
  Demon,
  FireworkBurst,
  Flame,
  Grid,
  HellTrain,
  Imp,
  LavaSea,
  PentagramSeal,
  SmokeTrail,
} from './art';
import {StampWord} from './ui';
import type {SceneProps} from './scenes1';

const W = 1280;
const H = 720;

const SKULL_ROWS = [
  '.WWWWWW.',
  'WWWWWWWW',
  'WBWWWWBW',
  'WBWWWWBW',
  'WWWWWWWW',
  '.WW.WW..',
  '.WBWBWB.',
  '.WWWWWW.',
];

// ---------------------------------------------------------------- BLUEPRINT (stanza 1)

export const BlueprintScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const chalk = '#d8e4dc';
  const reveal = interpolate(f, [0, 40], [0, 1], {extrapolateRight: 'clamp'});
  return (
    <AbsoluteFill style={{background: `radial-gradient(ellipse at 50% 45%, #14201a 0%, #0a120e 75%)`}}>
      {/* chalk dust */}
      {Array.from({length: 16}, (_, i) => (
        <div key={i} style={{position: 'absolute', left: random(`cd-${i}`) * W, top: (random(`cd2-${i}`) * H + f * 0.6) % H, width: 3, height: 3, background: chalk, opacity: 0.25}} />
      ))}
      {/* wooden frame */}
      <div style={{position: 'absolute', inset: 18, border: '16px solid #4a3418', boxShadow: 'inset 0 0 60px rgba(0,0,0,0.7)'}} />
      {/* chalk train, technical elevation */}
      <div style={{position: 'absolute', left: 180, top: 210, filter: 'grayscale(1) brightness(2.6) contrast(0.8)', opacity: 0.92 * reveal}}>
        <HellTrain frame={f * 0.4} u={11} glow={0.3} />
      </div>
      {/* dimension arrows + labels */}
      <svg width={W} height={H} style={{position: 'absolute', inset: 0, opacity: reveal}}>
        <line x1={205} y1={560} x2={945} y2={560} stroke={chalk} strokeWidth={2} strokeDasharray="7 5" />
        <line x1={205} y1={548} x2={205} y2={572} stroke={chalk} strokeWidth={2} />
        <line x1={945} y1={548} x2={945} y2={572} stroke={chalk} strokeWidth={2} />
        <text x={575} y={592} fill={chalk} fontSize={24} fontFamily={SERIF} textAnchor="middle" fontStyle="italic">
          66 ft 6 in (patent pending)
        </text>
        <line x1={1000} y1={230} x2={1000} y2={430} stroke={chalk} strokeWidth={2} strokeDasharray="7 5" />
        <text x={1022} y={340} fill={chalk} fontSize={22} fontFamily={SERIF} fontStyle="italic">
          14 ft
        </text>
        <text x={1022} y={368} fill={chalk} fontSize={22} fontFamily={SERIF} fontStyle="italic">
          firebox
        </text>
        {/* annotation arrows */}
        <text x={330} y={180} fill={chalk} fontSize={22} fontFamily={SERIF} fontStyle="italic" transform="rotate(-4 330 180)">
          boilerplate: black as the Pit ✓
        </text>
        <line x1={430} y1={192} x2={500} y2={300} stroke={chalk} strokeWidth={1.5} />
        <text x={760} y={160} fill={chalk} fontSize={22} fontFamily={SERIF} fontStyle="italic" transform="rotate(3 760 160)">
          brimstone intake, bolted in brass
        </text>
        <line x1={850} y1={172} x2={800} y2={260} stroke={chalk} strokeWidth={1.5} />
      </svg>
      {/* chalk pentagram, of course */}
      <div style={{position: 'absolute', right: 90, bottom: 80, opacity: 0.85 * reveal}}>
        <PentagramSeal size={120} color={chalk} spin={f * 0.25} />
      </div>
      <div style={{position: 'absolute', left: 70, bottom: 58, fontFamily: SERIF, color: chalk, fontSize: 26, letterSpacing: 4, opacity: reveal}}>
        FIG. 1 — THE HELLRAIL <span style={{fontSize: 18, fontStyle: 'italic'}}>(specifications diabolical, warranty void)</span>
      </div>
      <StampWord from={20} word="ATTEND!" size={130} color={C.flame} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- BOILER (stanza 2)

export const BoilerScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  const p = punch(start + f);
  const psi = Math.min(800, Math.round(interpolate(f, [0, dur * 0.75], [0, 800]) / 10) * 10 + Math.round(p * 40));
  const needle = interpolate(psi, [0, 1000], [-120, 120]);
  return (
    <AbsoluteFill style={{background: `radial-gradient(ellipse at 30% 60%, #241008 0%, #0c0404 70%)`}}>
      {/* enormous train, cropped close */}
      <div style={{position: 'absolute', left: -260, top: 40, transform: `scale(${1 + e * 0.01})`}}>
        <HellTrain frame={f} u={19} glow={e} />
      </div>
      <SmokeTrail frame={f} x={860} y={90} scale={2.2} n={12} />
      {/* steam jets popping off rivets */}
      <svg width={W} height={H} style={{position: 'absolute', inset: 0}}>
        {Array.from({length: 6}, (_, i) => {
          const t = ((f * 3 + i * 33) % 90) / 90;
          const jx = 240 + random(`jet-${i}`) * 600;
          const jy = 280 + random(`jet2-${i}`) * 160;
          return <circle key={i} cx={jx + t * 30} cy={jy - t * 46} r={3 + t * 14} fill="#e8e4d8" opacity={(1 - t) * 0.5} />;
        })}
        {/* rivet sparks on the beat */}
        {p > 0.1
          ? Array.from({length: 8}, (_, i) => (
              <rect key={i} x={300 + random(`rs-${i}-${f}`) * 500} y={200 + random(`rs2-${i}-${f}`) * 300} width={5} height={5} fill={C.yellow} />
            ))
          : null}
      </svg>
      {/* pressure gauge */}
      <div style={{position: 'absolute', right: 70, top: 90, width: 260, height: 260}}>
        <svg width={260} height={260}>
          <circle cx={130} cy={130} r={120} fill="#181008" stroke={C.brass} strokeWidth={10} />
          {Array.from({length: 11}, (_, i) => {
            const a = ((-120 + i * 24) * Math.PI) / 180;
            return (
              <line
                key={i}
                x1={130 + Math.sin(a) * 96}
                y1={130 - Math.cos(a) * 96}
                x2={130 + Math.sin(a) * 108}
                y2={130 - Math.cos(a) * 108}
                stroke={i >= 8 ? C.red : C.bone}
                strokeWidth={i % 5 === 0 ? 5 : 2.5}
              />
            );
          })}
          <text x={130} y={190} fill={C.bone} fontSize={22} fontFamily={SERIF} textAnchor="middle">
            LBS / PIPE
          </text>
          <line
            x1={130}
            y1={130}
            x2={130 + Math.sin((needle * Math.PI) / 180 + (p * 8 * Math.PI) / 180) * 92}
            y2={130 - Math.cos((needle * Math.PI) / 180 + (p * 8 * Math.PI) / 180) * 92}
            stroke={C.red}
            strokeWidth={6}
            strokeLinecap="round"
          />
          <circle cx={130} cy={130} r={12} fill={C.brass} />
        </svg>
        <div style={{textAlign: 'center', fontFamily: SERIF, color: psi >= 800 ? C.red : C.flame, fontSize: 44, fontWeight: 900, marginTop: 6, textShadow: '0 0 20px rgba(232,88,24,0.6)'}}>
          {psi} PSI
        </div>
      </div>
      {psi >= 790 ? (
        <div style={{position: 'absolute', right: 60, top: 440, fontFamily: SERIF, color: C.red, fontSize: 26, fontWeight: 900, letterSpacing: 3, border: `4px solid ${C.red}`, padding: '6px 14px', transform: `rotate(-6deg)`, opacity: 0.6 + Math.abs(Math.sin(f / 4)) * 0.4}}>
          EXCEEDING SPEC ✓
        </div>
      ) : null}
      <StampWord from={Math.round(dur * 0.35)} word="TRIPLE-THICK" size={100} color={C.gold} />
      <StampWord from={Math.round(dur * 0.8)} word="BOUND!" size={140} color={C.ember} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- WHEELS (stanza 3)

export const WheelsScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  const bob = Math.sin(f / 5) * 4;
  const tilt = interpolate(f, [0, dur], [0, 5]);
  return (
    <AbsoluteFill style={{background: `linear-gradient(180deg, ${C.ink} 0%, ${C.blood} 70%, ${C.red} 100%)`, transform: `rotate(${tilt * 0.3}deg)`}}>
      <Crags w={W} h={260} seed="wfar" color="#1c0c0a" y={H - 380} drift={f * 2.2} />
      <Crags w={W} h={200} seed="wnear" color="#12080a" y={H - 300} drift={f * 5} />
      <LavaSea frame={f * 2} w={W} h={150} top={H - 150} />
      {/* speed lines */}
      {Array.from({length: 16}, (_, i) => {
        const y = random(`sl-${i}`) * H;
        const len = 90 + random(`sl2-${i}`) * 200;
        const x = W - ((f * (28 + random(`sl3-${i}`) * 26) + random(`sl4-${i}`) * W) % (W + len));
        return <div key={i} style={{position: 'absolute', left: x, top: y, width: len, height: 2.5, background: C.bone, opacity: 0.18 + e * 0.15}} />;
      })}
      {/* the train, wheels furious */}
      <div style={{position: 'absolute', left: 210, top: 330 + bob}}>
        <HellTrain frame={f * 2.6} u={10} glow={e} />
      </div>
      <SmokeTrail frame={f * 1.6} x={720} y={350 + bob} scale={1.7} n={11} />
      {/* wheel sparks */}
      <svg width={W} height={H} style={{position: 'absolute', inset: 0}}>
        {Array.from({length: 14}, (_, i) => {
          const t = ((f * 5 + i * 16) % 60) / 60;
          return (
            <rect
              key={i}
              x={420 + random(`ws-${i}`) * 260 - t * 260}
              y={580 + Math.sin(t * 8 + i) * 18 - t * 60 * random(`ws2-${i}`)}
              width={6 * (1 - t) + 2}
              height={3}
              fill={t < 0.4 ? C.yellow : C.ember}
              opacity={1 - t}
            />
          );
        })}
      </svg>
      <StampWord from={Math.round(dur * 0.18)} word="30 TONS" size={130} color={C.gold} />
      <StampWord from={Math.round(dur * 0.62)} word="LOSS OF TRACTION?" size={80} color={C.red} />
      <StampWord from={Math.round(dur * 0.62) + 34} word="JUST A WHIM!" size={110} color={C.flame} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- LUXURY (stanza 5)

export const LuxuryScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  const sway = Math.sin(f / 13) * 3;
  const gimbal = Math.sin(f / 13) * 10; // champagne stays level while the car rocks
  return (
    <AbsoluteFill style={{background: `linear-gradient(180deg, #38080c 0%, #4a0c10 55%, #240608 100%)`, transform: `rotate(${sway * 0.15}deg)`}}>
      {/* damask dots */}
      {Array.from({length: 40}, (_, i) => (
        <div key={i} style={{position: 'absolute', left: (i % 8) * 165 + 40, top: Math.floor(i / 8) * 130 + 30, width: 34, height: 34, borderRadius: '50%', border: `2px solid rgba(232,168,50,0.18)`, transform: 'rotate(45deg)'}} />
      ))}
      {/* gold trim */}
      <div style={{position: 'absolute', left: 0, right: 0, top: 96, height: 7, background: C.brass, opacity: 0.8}} />
      <div style={{position: 'absolute', left: 0, right: 0, bottom: 140, height: 7, background: C.brass, opacity: 0.8}} />
      {/* window with hellscape rushing past */}
      <div style={{position: 'absolute', right: 90, top: 150, width: 330, height: 260, border: `16px solid #6a4828`, borderRadius: 14, overflow: 'hidden', background: `linear-gradient(180deg, ${C.ink}, ${C.blood})`}}>
        <Crags w={330} h={120} seed="lux1" color="#160a08" y={90} drift={f * 6} />
        <LavaSea frame={f * 2} w={330} h={70} top={190} />
        <div style={{position: 'absolute', top: 8, left: 10, fontFamily: SERIF, fontSize: 14, color: C.bone, opacity: 0.7, fontStyle: 'italic'}}>
          scenery: purgatorial (upgradeable)
        </div>
      </div>
      {/* chandelier */}
      <div style={{position: 'absolute', left: 300, top: 60, transform: `rotate(${sway}deg)`, transformOrigin: 'top center'}}>
        <div style={{width: 6, height: 60, background: C.brass, margin: '0 auto'}} />
        <div style={{width: 190, height: 10, background: C.brass, borderRadius: 5}} />
        {[0, 80, 160].map((dx) => (
          <div key={dx} style={{position: 'absolute', left: dx - 3, top: 40}}>
            <Flame frame={f} seed={`lux-${dx}`} u={3.4} />
          </div>
        ))}
      </div>
      {/* plush seat + skull passenger in a top hat */}
      <div style={{position: 'absolute', left: 120, bottom: 150}}>
        <div style={{width: 300, height: 190, background: '#701420', borderRadius: '18px 18px 8px 8px', boxShadow: 'inset 0 14px 0 rgba(0,0,0,0.25)'}} />
        <div style={{position: 'absolute', left: 0, bottom: 0, width: 300, height: 80, background: '#8a1a28', borderRadius: 10, boxShadow: 'inset 0 8px 0 rgba(0,0,0,0.2)'}} />
        {[30, 105, 180, 255].map((bx) => (
          <div key={bx} style={{position: 'absolute', left: bx, top: 30, width: 10, height: 10, borderRadius: '50%', background: C.gold}} />
        ))}
        <div style={{position: 'absolute', left: 95, top: -74}}>
          <svg width={110} height={110} viewBox="0 0 11 11" style={{shapeRendering: 'crispEdges', overflow: 'visible'}}>
            <rect x={1} y={-2.4} width={9} height={1.2} fill={C.ink} />
            <rect x={2.6} y={-6} width={5.8} height={3.8} fill={C.ink} />
            <rect x={2.6} y={-3.3} width={5.8} height={0.9} fill={C.blood} />
            <g transform="translate(1.5 0)">
              <Grid rows={SKULL_ROWS} pal={{W: C.bone, B: C.ink}} />
            </g>
          </svg>
        </div>
      </div>
      {/* gimballed champagne — glass swings to stay level */}
      <div style={{position: 'absolute', left: 660, bottom: 210, transform: `rotate(${-gimbal * 0.4}deg)`, transformOrigin: 'top center'}}>
        <svg width={120} height={210} style={{overflow: 'visible'}}>
          <rect x={54} y={0} width={5} height={40} fill={C.brass} />
          <circle cx={56.5} cy={48} r={9} fill="none" stroke={C.brass} strokeWidth={4} />
          <g transform={`rotate(${gimbal * 0.9} 56.5 60)`}>
            <polygon points="34,58 79,58 63,120 63,168 50,168 50,120" fill="rgba(232,220,192,0.3)" stroke={C.bone} strokeWidth={2.5} />
            <rect x={37} y={60} width={39} height={26} fill={C.goldPale} opacity={0.85} />
            {[0, 1].map((i) => (
              <circle key={i} cx={48 + i * 14} cy={80 - ((f * 2 + i * 17) % 26)} r={2.6} fill={C.white} opacity={0.85} />
            ))}
            <rect x={44} y={166} width={25} height={7} fill="rgba(232,220,192,0.5)" />
          </g>
        </svg>
        <div style={{fontFamily: SERIF, fontSize: 15, color: C.goldPale, fontStyle: 'italic', textAlign: 'center'}}>gimballed</div>
      </div>
      {/* porter demon with tray */}
      <div style={{position: 'absolute', right: 480, bottom: 140}}>
        <Demon u={9} frame={f} variant="magnate" flip />
        <div style={{position: 'absolute', left: -60, top: 92, width: 90, height: 8, background: C.brass, borderRadius: 4}} />
        <div style={{position: 'absolute', left: -44, top: 74, width: 22, height: 18, background: C.bone, borderRadius: '50% 50% 0 0'}} />
      </div>
      <StampWord from={Math.round(dur * 0.3)} word="PLUSHEST PERDITION" size={82} color={C.gold} />
      <StampWord from={Math.round(dur * 0.75)} word="LUMINOUS LEAD✦" size={90} color={C.goldPale} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- VELOCITY (stanza 6)

export const VelocityScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  const p = punch(start + f);
  const speed = Math.min(1000, Math.round(interpolate(f, [0, dur * 0.7], [0, 1000])));
  const needleA = interpolate(speed, [0, 1000], [-115, 115]) + p * 14;
  return (
    <AbsoluteFill style={{background: `linear-gradient(160deg, #0c0406 0%, ${C.demonDark} 80%)`}}>
      {/* diagonal speed streaks */}
      {Array.from({length: 22}, (_, i) => {
        const len = 140 + random(`vs-${i}`) * 260;
        const xx = W + 200 - ((f * (30 + random(`vs2-${i}`) * 30) + random(`vs3-${i}`) * (W + 400)) % (W + 400 + len));
        const yy = random(`vs4-${i}`) * H;
        return <div key={i} style={{position: 'absolute', left: xx, top: yy, width: len, height: 3, background: i % 3 ? C.ember : C.bone, opacity: 0.22, transform: 'rotate(18deg)'}} />;
      })}
      {/* the train plunging diagonally */}
      <div style={{position: 'absolute', left: 340 + Math.sin(f / 6) * 8, top: 250, transform: 'rotate(18deg)'}}>
        <HellTrain frame={f * 3} u={10.5} glow={Math.min(1, e * 1.3)} />
      </div>
      <SmokeTrail frame={f * 2} x={880} y={210} scale={1.9} n={12} />
      {/* speedometer */}
      <div style={{position: 'absolute', left: 60, top: 60}}>
        <svg width={300} height={300}>
          <circle cx={150} cy={150} r={138} fill="rgba(10,5,5,0.85)" stroke={C.brass} strokeWidth={9} />
          {Array.from({length: 11}, (_, i) => {
            const a = ((-115 + i * 23) * Math.PI) / 180;
            return (
              <g key={i}>
                <line x1={150 + Math.sin(a) * 112} y1={150 - Math.cos(a) * 112} x2={150 + Math.sin(a) * 126} y2={150 - Math.cos(a) * 126} stroke={C.bone} strokeWidth={i % 5 === 0 ? 5 : 2} />
                {i % 5 === 0 ? (
                  <text x={150 + Math.sin(a) * 92} y={156 - Math.cos(a) * 92} fill={C.bone} fontSize={20} fontFamily={SERIF} textAnchor="middle">
                    {i * 100}
                  </text>
                ) : null}
              </g>
            );
          })}
          <text x={150} y={205} fill={C.flame} fontSize={19} fontFamily={SERIF} textAnchor="middle" letterSpacing={2}>
            MILES DOWN
          </text>
          <text x={150} y={228} fill={C.bone} fontSize={15} fontFamily={SERIF} textAnchor="middle" fontStyle="italic">
            per tea-time
          </text>
          <line x1={150} y1={150} x2={150 + Math.sin((needleA * Math.PI) / 180) * 108} y2={150 - Math.cos((needleA * Math.PI) / 180) * 108} stroke={C.red} strokeWidth={7} strokeLinecap="round" />
          <circle cx={150} cy={150} r={13} fill={C.brass} />
        </svg>
      </div>
      {/* the tea in question */}
      <div style={{position: 'absolute', right: 110, bottom: 120}}>
        <svg width={170} height={150} style={{overflow: 'visible'}}>
          {[0, 1].map((i) => (
            <path key={i} d={`M ${58 + i * 26} ${44 - ((f + i * 12) % 36)} q 8 -10 0 -20`} fill="none" stroke={C.bone} strokeWidth={3.5} opacity={0.5 - ((f + i * 12) % 36) / 90} />
          ))}
          <path d="M 30 52 L 122 52 L 112 108 L 40 108 Z" fill={C.bone} />
          <path d="M 122 60 q 26 4 0 30" fill="none" stroke={C.bone} strokeWidth={8} />
          <rect x={36} y={58} width={80} height={10} fill="#6a4020" />
          <ellipse cx={76} cy={124} rx={64} ry={10} fill={C.bone} />
        </svg>
        <div style={{fontFamily: SERIF, fontSize: 16, color: C.bone, fontStyle: 'italic', textAlign: 'center'}}>your tea (still hot on arrival)</div>
      </div>
      {/* receipts raining */}
      {Array.from({length: 9}, (_, i) => {
        const x = random(`rc-${i}`) * (W - 300) + 150;
        const y = ((random(`rc2-${i}`) * H + f * (4 + random(`rc3-${i}`) * 4)) % (H + 120)) - 80;
        return (
          <div key={i} style={{position: 'absolute', left: x, top: y, width: 60, height: 84, background: C.bone, transform: `rotate(${(random(`rc4-${i}`) - 0.5) * 40 + f}deg)`, padding: 5, fontFamily: SERIF, fontSize: 9, color: C.soot, lineHeight: 1.5}}>
            RECEIPT
            <br />
            √ math
            <br />
            √ checked
            <br />— {Math.round(random(`rc5-${i}`) * 900 + 100)} mi
          </div>
        );
      })}
      <StampWord from={Math.round(dur * 0.28)} word="VISCERA-TESTED" size={92} color={C.flame} />
      <StampWord from={Math.round(dur * 0.85)} word="DO THE MATH!" size={110} color={C.gold} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- FUEL (stanza 7)

const FUEL_DOCS = ['DEED', 'BOND', 'NOTE', 'IOU', 'WILL', 'LEASE'];

export const FuelScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  return (
    <AbsoluteFill style={{background: `linear-gradient(180deg, #060202 0%, #1c0604 60%, #380c04 100%)`}}>
      {/* burning farm on the hill */}
      <Crags w={W} h={200} seed="fuelhill" color="#0c0404" y={H - 330} drift={0} />
      <div style={{position: 'absolute', left: 120, top: 330}}>
        <svg width={170} height={120} style={{shapeRendering: 'crispEdges', overflow: 'visible'}}>
          <rect x={20} y={54} width={80} height={50} fill="#180a06" />
          <polygon points="12,56 60,18 108,56" fill="#221008" />
          <rect x={72} y={74} width={18} height={30} fill="#2c130a" />
          <rect x={34} y={66} width={16} height={14} fill={C.orange} opacity={0.8 + Math.sin(f / 4) * 0.2} />
          <rect x={120} y={40} width={8} height={64} fill="#180a06" />
          <g transform={`rotate(${f * 1.6} 124 40)`}>
            {[0, 90, 180, 270].map((a) => (
              <rect key={a} x={122} y={12} width={4} height={28} fill="#221008" transform={`rotate(${a} 124 40)`} />
            ))}
          </g>
        </svg>
        <div style={{position: 'absolute', left: 30, top: -34}}>
          <Flame frame={f} seed="farm1" u={5} />
        </div>
        <div style={{position: 'absolute', left: 70, top: -20}}>
          <Flame frame={f + 7} seed="farm2" u={4} />
        </div>
        <div style={{fontFamily: SERIF, fontSize: 15, color: C.ash, fontStyle: 'italic', marginTop: 4}}>lot 44: foreclosed (fragrant)</div>
      </div>
      {/* THE FURNACE MOUTH */}
      <div style={{position: 'absolute', left: W / 2 - 230, bottom: 0, width: 460, height: 320}}>
        <svg width={460} height={320}>
          <path d="M 30 320 L 30 150 Q 230 -40 430 150 L 430 320 Z" fill="#0a0404" stroke={C.iron} strokeWidth={10} />
          <path d="M 60 320 L 60 168 Q 230 10 400 168 L 400 320 Z" fill={C.ember} opacity={0.5 + e * 0.5} />
          <path d="M 100 320 L 100 190 Q 230 70 360 190 L 360 320 Z" fill={C.flame} opacity={0.5 + e * 0.5} />
          {/* teeth, because the furnace is also alive */}
          {[110, 170, 230, 290, 350].map((tx) => (
            <polygon key={tx} points={`${tx - 20},110 ${tx},160 ${tx + 20},110`} fill="#0a0404" transform={`translate(0 ${Math.sin(f / 9 + tx) * 4})`} />
          ))}
        </svg>
        <div style={{position: 'absolute', left: 150, top: 130, transform: 'scale(2.6)'}}>
          <Flame frame={f} seed="mouth" u={6} />
        </div>
      </div>
      {/* documents arcing into the mouth */}
      {Array.from({length: 8}, (_, i) => {
        const t = ((f * 1.6 + i * 21) % 90) / 90;
        const fromLeft = i % 2 === 0;
        const x0 = fromLeft ? 150 : W - 180;
        const x1 = W / 2 + (fromLeft ? -40 : 40);
        const x = x0 + (x1 - x0) * t;
        const y = 470 - Math.sin(t * Math.PI) * 260 + t * 60;
        return (
          <div key={i} style={{position: 'absolute', left: x, top: y, width: 52, height: 66, background: C.bone, opacity: t > 0.9 ? (1 - t) * 10 : 1, transform: `rotate(${t * 400 * (fromLeft ? 1 : -1)}deg)`, fontFamily: SERIF, fontSize: 12, fontWeight: 700, color: C.soot, textAlign: 'center', paddingTop: 22}}>
            {FUEL_DOCS[i % FUEL_DOCS.length]}
          </div>
        );
      })}
      {/* imps shovelling */}
      {[0, 1].map((i) => {
        const shovel = Math.sin(f / 7 + i * 2);
        return (
          <div key={i} style={{position: 'absolute', left: i === 0 ? W / 2 - 330 : W / 2 + 260, bottom: 60, transform: `rotate(${shovel * 14 * (i === 0 ? 1 : -1)}deg) scaleX(${i === 0 ? 1 : -1})`}}>
            <Imp u={6} hop={Math.abs(shovel) * 8} />
            <div style={{position: 'absolute', left: 48, top: 44, width: 66, height: 5, background: '#6a4828', transform: 'rotate(-24deg)'}} />
            <div style={{position: 'absolute', left: 100, top: 18, width: 22, height: 16, background: '#2c1c10', borderRadius: '0 0 8px 8px'}} />
          </div>
        );
      })}
      {/* embers rising */}
      {Array.from({length: 14}, (_, i) => {
        const t = ((f * 2 + i * 19) % 130) / 130;
        return (
          <div key={i} style={{position: 'absolute', left: W / 2 - 150 + random(`em-${i}`) * 300 + Math.sin(t * 7 + i) * 26, top: 560 - t * 480, width: 5, height: 5, background: t < 0.5 ? C.yellow : C.ember, opacity: 1 - t, borderRadius: '50%'}} />
        );
      })}
      {/* BTU meter */}
      <div style={{position: 'absolute', right: 30, bottom: 34, fontFamily: SERIF, color: C.bone, background: 'rgba(10,5,5,0.8)', border: `2px solid ${C.brass}`, padding: '10px 18px', fontSize: 24, letterSpacing: 2, textAlign: 'right'}}>
        CALORIFIC CONTENT OF GREED
        <br />
        <span style={{color: C.flame, fontSize: 40, fontWeight: 900}}>
          {(6666 + Math.round(interpolate(f, [0, dur], [0, 93333]) / 100) * 100).toLocaleString('en-GB')} BTU
        </span>
      </div>
      <StampWord from={Math.round(dur * 0.3)} word="100% RENEWABLE*" size={84} color={C.flame} />
      <StampWord from={Math.round(dur * 0.32) + 40} word="*ruin renews itself" size={44} color={C.bone} />
      <StampWord from={Math.round(dur * 0.82)} word="BLISTERING SPEED" size={92} color={C.red} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- FINALE (stanza 8)

export const FinaleScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  const nBankers = Math.min(6, 1 + Math.floor(f / 110));
  const countdown = Math.max(0, 3599 - Math.floor((f / 30) * 60));
  const mm = String(Math.floor(countdown / 60)).padStart(2, '0');
  const ss = String(countdown % 60).padStart(2, '0');
  const certSpin = interpolate(f, [dur * 0.55, dur * 0.62], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const ticker = 'SHARES SELLING SWIFTLY ⬆ SUBSCRIBE AND BE SAVED ⬆ STAMPS AT THE STATION ⬆ DON\'T WAIT ⬆ GOLD BY THE GUILDER, THE GUINEA, THE GROAT ⬆ INVEST AND INCINERATE FATE ⬆ ';
  return (
    <AbsoluteFill style={{background: `linear-gradient(180deg, #2a2018 0%, #3a2c20 70%, #241a12 100%)`}}>
      {[80, 340, 600, 860, 1120].map((x) => (
        <div key={x} style={{position: 'absolute', left: x, top: 60, width: 58, height: 520}}>
          <div style={{position: 'absolute', inset: 0, background: `linear-gradient(90deg, #6a5c48, #a89878 40%, #6a5c48)`}} />
          <div style={{position: 'absolute', top: -14, left: -10, right: -10, height: 16, background: '#a89878'}} />
          <div style={{position: 'absolute', bottom: -14, left: -10, right: -10, height: 16, background: '#a89878'}} />
        </div>
      ))}
      <div style={{position: 'absolute', left: 0, right: 0, bottom: 0, height: 150, background: `repeating-conic-gradient(#241a12 0% 25%, #3a2e20 0% 50%) 0 0 / 90px 46px`}} />
      {/* fireworks over the trading floor, why not */}
      {Array.from({length: 3}, (_, i) => {
        const cyc = Math.floor((f + i * 34) / 100);
        const t = ((f + i * 34) % 100) / 100;
        return <FireworkBurst key={i} t={t} x={160 + random(`ffw-${cyc}-${i}`) * (W - 320)} y={80 + random(`ffw2-${cyc}-${i}`) * 140} color={[C.gold, C.ember, C.bone][i]} size={70} />;
      })}
      {/* ticker */}
      <div style={{position: 'absolute', top: 12, left: 0, right: 0, overflow: 'hidden', background: 'rgba(10,5,5,0.9)', borderTop: `2px solid ${C.gold}`, borderBottom: `2px solid ${C.gold}`, height: 42}}>
        <div style={{whiteSpace: 'nowrap', fontFamily: SERIF, fontSize: 26, lineHeight: '40px', letterSpacing: 2, color: C.gold, transform: `translateX(${-((f * 4.2) % 3000)}px)`}}>
          {ticker.repeat(4)}
        </div>
      </div>
      {/* demon bankers frantically multiplying */}
      {Array.from({length: nBankers}, (_, i) => {
        const col = i % 6;
        return (
          <div key={i} style={{position: 'absolute', left: 100 + col * 195, top: 320, transformOrigin: 'bottom center'}}>
            <Demon u={7.6} frame={f * 1.5 + i * 7} variant={i === 2 ? 'magnate' : 'banker'} flip={i % 2 === 1} />
          </div>
        );
      })}
      {/* imps going wild on the floor */}
      {Array.from({length: 7}, (_, i) => (
        <div key={i} style={{position: 'absolute', left: 80 + i * 170, bottom: 60}}>
          <Imp u={4.6} hop={Math.abs(Math.sin(f / 4 + i * 1.1)) * 34 * (0.4 + e)} flip={i % 2 === 0} />
        </div>
      ))}
      {/* double coin fountain */}
      {Array.from({length: 18}, (_, i) => {
        const t = ((f * 2.4 + i * 12) % 90) / 90;
        const side = i % 2 === 0 ? 380 : W - 380;
        const spread = (random(`fcf-${i}`) - 0.5) * 420;
        const cy = 560 - Math.sin(t * Math.PI) * 360;
        return (
          <div key={i} style={{position: 'absolute', left: side + spread * t - 14, top: cy, opacity: 1 - t * 0.3}}>
            <Coin u={4.2} rot={f * 7 + i * 30} />
          </div>
        );
      })}
      {/* departure countdown */}
      <div style={{position: 'absolute', right: 40, top: 76, fontFamily: SERIF, background: 'rgba(10,5,5,0.88)', border: `3px solid ${C.red}`, padding: '10px 22px', textAlign: 'center'}}>
        <div style={{color: C.bone, fontSize: 20, letterSpacing: 3}}>DEPARTS ON THE HOUR</div>
        <div style={{color: C.red, fontSize: 52, fontWeight: 900, letterSpacing: 4, opacity: 0.75 + Math.abs(Math.sin(f / 6)) * 0.25}}>
          {mm}:{ss}
        </div>
      </div>
      {/* the share certificate descends, spinning */}
      {certSpin > 0 ? (
        <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center'}}>
          <div
            style={{
              transform: `scale(${certSpin}) rotate(${(1 - certSpin) * 720 + Math.sin(f / 18) * 3}deg)`,
              width: 560,
              background: C.parchment,
              border: `6px solid ${C.brass}`,
              boxShadow: '0 0 80px rgba(232,168,50,0.5)',
              padding: '26px 34px',
              textAlign: 'center',
              fontFamily: SERIF,
              color: C.soot,
            }}
          >
            <div style={{fontSize: 30, letterSpacing: 5, fontWeight: 900, color: C.demonDark}}>CERTIFICATE OF SHARES</div>
            <div style={{fontSize: 19, fontStyle: 'italic', marginTop: 6}}>The Hellrail Consolidated Infernal Railway Co.</div>
            <div style={{display: 'flex', justifyContent: 'center', margin: '12px 0'}}>
              <PentagramSeal size={84} color={C.blood} spin={f * 0.8} />
            </div>
            <div style={{fontSize: 18, lineHeight: 1.6}}>
              entitles the bearer to <b>ONE (1) DOWER, COMBUSTIBLE</b>
              <br />
              signed, sealed, certified &amp; stoked — <i>the Conductors are calling</i>
            </div>
          </div>
        </AbsoluteFill>
      ) : null}
      <StampWord from={Math.round(dur * 0.08)} word="SUBSCRIBE!" size={120} color={C.red} />
      <StampWord from={Math.round(dur * 0.2)} word="DON'T WAIT!" size={110} color={C.gold} />
      <StampWord from={Math.round(dur * 0.42)} word="INCINERATE FATE!" size={86} color={C.ember} />
      <StampWord from={Math.round(dur * 0.86)} word="CLIMB ON!" size={120} color={C.red} />
      <StampWord from={Math.round(dur * 0.93)} word="CASH IN!" size={130} color={C.flame} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- END CARD

export const EndCardScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const inO = interpolate(f, [0, 18], [0, 1], {extrapolateRight: 'clamp'});
  const outO = interpolate(f, [dur - 26, dur - 4], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  return (
    <AbsoluteFill style={{background: C.ink, alignItems: 'center', justifyContent: 'center', fontFamily: SERIF, textAlign: 'center'}}>
      <div style={{opacity: inO * outO}}>
        <div style={{display: 'flex', justifyContent: 'center', marginBottom: 22}}>
          <PentagramSeal size={90} color={C.blood} spin={f * 0.3} />
        </div>
        <div style={{fontSize: 62, fontWeight: 900, letterSpacing: 8, color: C.blood, textShadow: `0 0 30px ${C.ember}`}}>THE HELLRAIL</div>
        <div style={{fontSize: 26, letterSpacing: 4, color: C.bone, marginTop: 10}}>DEPARTS ON THE HOUR</div>
        <div style={{fontSize: 15, letterSpacing: 2, color: C.ash, marginTop: 30, lineHeight: 1.9}}>
          a frenetic advertisement in verse · terms apply in perpetuity
          <br />
          souls priced pro rata · past performance is a guarantee of future results
          <br />
          the friction coefficient remains a laugh
        </div>
      </div>
    </AbsoluteFill>
  );
};
