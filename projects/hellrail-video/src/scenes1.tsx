import React from 'react';
import {AbsoluteFill, interpolate, random, useCurrentFrame} from 'remotion';
import {C, SERIF} from './theme';
import {energy} from './audio';
import {
  Coin,
  Crags,
  Demon,
  FireworkBurst,
  HellTrain,
  Imp,
  LavaSea,
  PentagramSeal,
  Skyline,
  SmokeTrail,
  StockChart,
  Widow,
} from './art';
import {LyricCard, PosterFrame, StampWord} from './ui';

const W = 1280;
const H = 720;

export type SceneProps = {start: number; dur: number};

// ---------------------------------------------------------------- 1. TITLE

export const TitleScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  const flicker = 0.75 + e * 0.35;
  const trainX = interpolate(f, [dur - 46, dur - 6], [W + 200, -700], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });
  const fall = interpolate(f, [dur - 20, dur], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const show = (at: number) =>
    ({
      opacity: interpolate(f, [at, at + 10], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'}),
      transform: `translateY(${interpolate(f, [at, at + 10], [18, 0], {
        extrapolateLeft: 'clamp',
        extrapolateRight: 'clamp',
      })}px)`,
    }) as React.CSSProperties;
  return (
    <AbsoluteFill style={{background: C.ink}}>
      <AbsoluteFill
        style={{
          background: `radial-gradient(ellipse at 50% 42%, ${C.parchment} 0%, ${C.parchmentDark} 55%, #6a5030 100%)`,
          transform: `rotate(${fall * 8}deg) translateY(${fall * fall * 760}px)`,
          alignItems: 'center',
          fontFamily: SERIF,
          textAlign: 'center',
          color: C.soot,
        }}
      >
        <PosterFrame color={C.blood} />
        <div style={{marginTop: 58, fontSize: 25, letterSpacing: 5, ...show(15)}}>
          MESSRS. BEELZEBUB, GRINDER &amp; CO. PRESENT
        </div>
        <div
          style={{
            fontSize: 128,
            fontWeight: 900,
            letterSpacing: 10,
            marginTop: 8,
            color: C.blood,
            textShadow: `0 0 ${22 * flicker}px ${C.ember}, 0 0 ${52 * flicker}px ${C.orange}, 0 4px 0 ${C.demonDark}`,
            ...show(42),
          }}
        >
          THE HELLRAIL
        </div>
        <div style={{fontSize: 30, letterSpacing: 7, marginTop: 4, ...show(75)}}>
          ⚒ A PROSPECTUS IN VERSE ⚒
        </div>
        <div style={{fontSize: 24, letterSpacing: 3, marginTop: 26, fontStyle: 'italic', ...show(105)}}>
          A Torrid Tunnel Through the Planet's Crust
        </div>
        <div style={{fontSize: 33, letterSpacing: 4, marginTop: 24, fontWeight: 700, color: C.demonDark, ...show(140)}}>
          GUARANTEED* RETURNS OF 6.66%
        </div>
        <div style={{fontSize: 14, letterSpacing: 1, marginTop: 8, opacity: 0.75, ...show(152)}}>
          *guarantee void where prohibited, i.e. everywhere
        </div>
        <div style={{position: 'absolute', right: 74, bottom: 62, opacity: 0.9, ...show(120)}}>
          <PentagramSeal size={110} color={C.blood} spin={f * 0.4} />
        </div>
        <div style={{position: 'absolute', left: 74, bottom: 74, fontSize: 15, fontStyle: 'italic', opacity: 0.7, textAlign: 'left', ...show(160)}}>
          Shares priced at £1/1s/1d
          <br />
          or one (1) immortal soul, pro rata
        </div>
      </AbsoluteFill>
      {/* the train barges through the poster */}
      <div style={{position: 'absolute', left: trainX, top: 300}}>
        <HellTrain frame={f} u={9} glow={e} />
      </div>
      {f > dur - 46 && f < dur - 6 ? <SmokeTrail frame={f} x={trainX + 490} y={310} scale={1.4} /> : null}
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- 2. BOOM

export const BoomScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  const bob = Math.sin(f / 7) * 5;
  const trainX = 240 + Math.sin(f / 40) * 30;
  return (
    <AbsoluteFill style={{background: `linear-gradient(180deg, ${C.ink} 0%, ${C.blood} 62%, ${C.red} 100%)`}}>
      {/* rising stock chart ghosted in the sky */}
      <div style={{position: 'absolute', left: 80, top: 60, width: W - 160, height: 320, opacity: 0.4}}>
        <StockChart w={W - 160} h={320} progress={f / dur} color={C.gold} />
      </div>
      <Crags w={W} h={260} seed="far" color="#1c0c0a" y={H - 380} drift={f * 0.4} />
      <Crags w={W} h={200} seed="near" color="#12080a" y={H - 300} drift={f * 1.1} />
      <LavaSea frame={f} w={W} h={150} top={H - 150} />
      {/* coin rain */}
      {Array.from({length: 18}, (_, i) => {
        const x = random(`cn-${i}`) * W;
        const speed = 5 + random(`cs-${i}`) * 6;
        const y = ((random(`cy-${i}`) * H + f * speed) % (H + 60)) - 40;
        return (
          <div key={i} style={{position: 'absolute', left: x, top: y}}>
            <Coin u={4.4} rot={f * 4 + i * 40} />
          </div>
        );
      })}
      {/* the train, bobbing over the lava */}
      <div style={{position: 'absolute', left: trainX, top: 330 + bob}}>
        <HellTrain frame={f} u={9.4} glow={e} />
      </div>
      <SmokeTrail frame={f} x={trainX + 510} y={350 + bob} scale={1.5} />
      {/* imps chasing the money */}
      {[0, 1, 2].map((i) => (
        <div key={i} style={{position: 'absolute', left: 90 + i * 90 + Math.sin(f / 8 + i) * 14, top: 560}}>
          <Imp u={4.2} hop={Math.abs(Math.sin(f / 5 + i * 1.4)) * 22 * (0.4 + e)} flip={i % 2 === 0} />
        </div>
      ))}
      <StampWord from={Math.round(dur * 0.3)} word="6.66% PREFERRED" size={92} color={C.gold} />
      <StampWord from={Math.round(dur * 0.72)} word="NO RISK†" size={120} color={C.red} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- 3. INVESTORS

export const InvestorsScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  const nBankers = Math.min(12, 2 ** Math.floor(f / 150));
  const widowX = interpolate(f, [420, 820], [-160, W + 60], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });
  const blueprint = interpolate(f, [180, 210], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'}) *
    interpolate(f, [330, 360], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const ticker = 'HRL ▲ 666 ⬆ SHARES SOARING SKYWARD ⬆ WATERED STOCK NOW ON TAP ⬆ WIDOWS & ORPHANS WELCOME ⬆ SILVER STREAMERS OVER THE STRAND ⬆ ';
  return (
    <AbsoluteFill style={{background: `linear-gradient(180deg, #2a2018 0%, #3a2c20 70%, #241a12 100%)`}}>
      {/* marble columns */}
      {[80, 340, 600, 860, 1120].map((x) => (
        <div key={x} style={{position: 'absolute', left: x, top: 60, width: 58, height: 520}}>
          <div style={{position: 'absolute', inset: 0, background: `linear-gradient(90deg, #6a5c48, #a89878 40%, #6a5c48)`}} />
          <div style={{position: 'absolute', top: -14, left: -10, right: -10, height: 16, background: '#a89878'}} />
          <div style={{position: 'absolute', bottom: -14, left: -10, right: -10, height: 16, background: '#a89878'}} />
        </div>
      ))}
      {/* checkered floor */}
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          bottom: 0,
          height: 150,
          background: `repeating-conic-gradient(#241a12 0% 25%, #3a2e20 0% 50%) 0 0 / 90px 46px`,
        }}
      />
      {/* ticker tape */}
      <div style={{position: 'absolute', top: 12, left: 0, right: 0, overflow: 'hidden', background: 'rgba(10,5,5,0.9)', borderTop: `2px solid ${C.gold}`, borderBottom: `2px solid ${C.gold}`, height: 42}}>
        <div
          style={{
            whiteSpace: 'nowrap',
            fontFamily: SERIF,
            fontSize: 26,
            lineHeight: '40px',
            letterSpacing: 2,
            color: C.gold,
            transform: `translateX(${-((f * 3.4) % 2400)}px)`,
          }}
        >
          {ticker.repeat(4)}
        </div>
      </div>
      {/* multiplying demon bankers */}
      {Array.from({length: nBankers}, (_, i) => {
        const col = i % 6;
        const row = Math.floor(i / 6);
        const appear = Math.min(1, Math.max(0, (f - Math.floor(Math.log2(Math.max(1, i + 1))) * 150) / 8));
        return (
          <div
            key={i}
            style={{
              position: 'absolute',
              left: 110 + col * 190 + (row ? 90 : 0),
              top: 300 + row * 130,
              transform: `scale(${appear * (1 + row * 0.35)})`,
              transformOrigin: 'bottom center',
            }}
          >
            <Demon u={row ? 8 : 6.4} frame={f + i * 7} variant={i === 2 ? 'magnate' : 'banker'} flip={i % 2 === 1} />
          </div>
        );
      })}
      {/* coin fountain */}
      {Array.from({length: 12}, (_, i) => {
        const t = ((f * 2 + i * 17) % 100) / 100;
        const spread = (random(`fc-${i}`) - 0.5) * 380;
        const cy = 560 - Math.sin(t * Math.PI) * 320;
        return (
          <div key={i} style={{position: 'absolute', left: W / 2 + spread * t - 14, top: cy, opacity: 1 - t * 0.4}}>
            <Coin u={4} rot={f * 6 + i * 30} />
          </div>
        );
      })}
      {/* the widow crosses with the barrow */}
      <div style={{position: 'absolute', left: widowX, top: 452}}>
        <Widow u={7.4} frame={f} />
      </div>
      {/* chalkboard blueprint interlude */}
      {blueprint > 0 ? (
        <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center', opacity: blueprint}}>
          <div style={{background: '#101a14', border: `14px solid #4a3418`, padding: 30, position: 'relative', transform: `rotate(-1.5deg)`}}>
            <PentagramSeal size={280} color={C.bone} spin={f * 0.2} />
            <div style={{position: 'absolute', top: 40, right: -160, fontFamily: SERIF, color: C.bone, fontSize: 20, fontStyle: 'italic', width: 150}}>
              radius: 666 ft
              <br />
              <br />
              est. cost:
              <br />
              all of it
            </div>
            <div style={{position: 'absolute', bottom: -4, left: 34, fontFamily: SERIF, color: C.bone, fontSize: 22, letterSpacing: 3}}>
              FIG. 1 — TUNNEL (DIABOLIC)
            </div>
          </div>
        </AbsoluteFill>
      ) : null}
      <LyricCard from={20} dur={120} text="THUNDEROUS TENDERS TOUTED HELLRAIL" />
      <LyricCard from={160} dur={130} text="ENGINEERS IN EVENING TAILCOATS · GASLIGHT'S GLARE" size={38} />
      <LyricCard from={205} dur={120} text="DIABOLIC DIAGRAMS — PENTAGRAMS WITH FLAIR" y="16%" size={38} />
      <LyricCard from={350} dur={110} text="MAGNATES MUNCHING MARZIPAN IN MARBLE HALLS" size={40} />
      <LyricCard from={480} dur={100} text="WATERED STOCK IN HAND" color={C.flame} />
      <LyricCard from={600} dur={150} text="WIDOWS WHEELED THEIR HUSBANDS' LEGACIES" size={40} />
      <StampWord from={140} word="BRAVO!" size={130} color={C.gold} />
      <StampWord from={545} word="SUBSCRIBE!" size={110} color={C.red} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- 4. LAUNCH

export const LaunchScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  const departT = interpolate(f, [430, dur - 60], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const trainX = 160 + departT * departT * (W + 500);
  const clink = e > 0.62 ? 1 : 0;
  return (
    <AbsoluteFill style={{background: `linear-gradient(180deg, #060810 0%, ${C.nightBlue} 60%, #1a1420 100%)`}}>
      {/* stars */}
      {Array.from({length: 30}, (_, i) => (
        <div
          key={i}
          style={{
            position: 'absolute',
            left: random(`st-${i}`) * W,
            top: random(`st2-${i}`) * 260,
            width: 3,
            height: 3,
            background: C.bone,
            opacity: 0.3 + 0.7 * Math.abs(Math.sin(f / 11 + i)),
          }}
        />
      ))}
      <Skyline w={W} h={300} color="#0a0c14" />
      {/* fireworks */}
      {Array.from({length: 4}, (_, i) => {
        const cyc = Math.floor((f + i * 30) / 90);
        const t = ((f + i * 30) % 90) / 90;
        return (
          <FireworkBurst
            key={i}
            t={t}
            x={140 + random(`fw-${cyc}-${i}`) * (W - 280)}
            y={70 + random(`fw2-${cyc}-${i}`) * 160}
            color={[C.gold, C.ember, C.bone, C.red][i]}
            size={60 + random(`fw3-${cyc}-${i}`) * 50}
          />
        );
      })}
      {/* bunting */}
      <svg width={W} height={90} style={{position: 'absolute', top: 250}}>
        {Array.from({length: 32}, (_, i) => {
          const bx = i * 41;
          const sway = Math.sin(f / 16 + i * 0.6) * 3;
          return (
            <polygon
              key={i}
              points={`${bx},${20 + Math.sin(i * 0.5) * 10} ${bx + 34},${20 + Math.sin((i + 1) * 0.5) * 10} ${bx + 17 + sway},${52 + Math.sin(i) * 6}`}
              fill={i % 3 === 0 ? C.red : i % 3 === 1 ? C.gold : C.bone}
              opacity={0.9}
            />
          );
        })}
      </svg>
      {/* platform */}
      <div style={{position: 'absolute', left: 0, right: 0, bottom: 0, height: 130, background: '#141018'}} />
      <div style={{position: 'absolute', left: 0, right: 0, bottom: 118, height: 12, background: '#2a2430'}} />
      {/* the train departs */}
      <div style={{position: 'absolute', left: trainX, top: 328}}>
        <HellTrain frame={f} u={8.6} glow={e} />
      </div>
      <SmokeTrail frame={f} x={trainX + 470} y={345} scale={1.2 + departT * 1.6} />
      {/* cheering imp crowd */}
      {Array.from({length: 9}, (_, i) => (
        <div key={i} style={{position: 'absolute', left: 60 + i * 135, bottom: 68}}>
          <Imp u={4.6} hop={Math.abs(Math.sin(f / 4.5 + i * 0.9)) * 30 * (0.3 + e)} flip={i % 2 === 0} />
        </div>
      ))}
      {/* giant champagne flutes clinking in the foreground */}
      <svg width={W} height={H} style={{position: 'absolute', inset: 0, overflow: 'visible'}}>
        <g transform={`translate(190 ${H - 20}) rotate(${8 + clink * 9 + Math.sin(f / 9) * 2})`}>
          <polygon points="-38,-260 38,-260 10,-150 10,-40 -10,-40 -10,-150" fill="rgba(232,220,192,0.35)" stroke={C.bone} strokeWidth={3} />
          <rect x={-34} y={-256} width={68} height={62} fill={C.goldPale} opacity={0.8} />
          {[0, 1, 2].map((i) => (
            <circle key={i} cx={-14 + i * 14} cy={-215 - ((f * 2 + i * 22) % 60)} r={3.4} fill={C.white} opacity={0.8} />
          ))}
          <rect x={-30} y={-38} width={60} height={10} fill="rgba(232,220,192,0.5)" />
        </g>
        <g transform={`translate(${W - 190} ${H - 20}) rotate(${-8 - clink * 9 - Math.sin(f / 9) * 2})`}>
          <polygon points="-38,-260 38,-260 10,-150 10,-40 -10,-40 -10,-150" fill="rgba(232,220,192,0.35)" stroke={C.bone} strokeWidth={3} />
          <rect x={-34} y={-256} width={68} height={62} fill={C.goldPale} opacity={0.8} />
          {[0, 1, 2].map((i) => (
            <circle key={i} cx={-14 + i * 14} cy={-215 - ((f * 2 + i * 26) % 60)} r={3.4} fill={C.white} opacity={0.8} />
          ))}
          <rect x={-30} y={-38} width={60} height={10} fill="rgba(232,220,192,0.5)" />
        </g>
      </svg>
      <LyricCard from={25} dur={115} text="FIRST DEPARTURE DAZZLED LONDON" />
      <LyricCard from={155} dur={115} text="DIAMOND SPARKS IN JETS OF GOLD CHAMPAGNE" size={40} />
      <LyricCard from={290} dur={115} text="STEEL RAILS RINGING · RED FLAGS FLYING" />
      <LyricCard from={425} dur={115} text="PRESSMEN PRAISED · PARSONS POCKETED" />
      <LyricCard from={560} dur={160} text="GOD AND MAMMON, SIDE BY SIDE" color={C.flame} size={54} y="42%" />
      <StampWord from={505} word="BON VOYAGE" size={110} color={C.gold} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- 5. DESCENT

const STRATA: Array<[string, string, number]> = [
  ['TOPSOIL', '#3a2c1c', 180],
  ['LONDON CLAY', '#4a3828', 220],
  ['BEDROCK', '#3a3430', 260],
  ['FOSSIL DEBT LAYER', '#2e2a2e', 260],
  ['MAGMA', '#701408', 280],
  ['MORAL BEDROCK (CRACKED)', '#241a20', 280],
  ['THE LEDGER LAYER', '#1a1414', 280],
  ['HELL PROPER — ARRIVALS', '#12060a', 420],
];

export const DescentScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  const worldH = STRATA.reduce((a, [, , h]) => a + h, 0);
  const camY = interpolate(f, [0, dur], [0, worldH - H]);
  const depth = Math.floor(interpolate(f, [0, dur], [1, 666]));
  const temp = Math.floor(interpolate(f, [0, dur], [98, 6660]));
  const cheese = Math.min(100, interpolate(f, [0, dur * 0.9], [0, 100]));
  let yAcc = 0;
  return (
    <AbsoluteFill style={{background: C.ink}}>
      <div style={{position: 'absolute', left: 0, top: -camY, width: W, height: worldH}}>
        {STRATA.map(([label, color, h], i) => {
          const y = yAcc;
          yAcc += h;
          return (
            <div key={i} style={{position: 'absolute', left: 0, top: y, width: W, height: h, background: color, borderBottom: `3px dashed rgba(232,220,192,0.25)`}}>
              <div style={{fontFamily: SERIF, color: C.bone, fontSize: 26, letterSpacing: 5, opacity: 0.75, position: 'absolute', left: 46, top: h / 2 - 16}}>
                {label}
              </div>
              <div style={{fontFamily: SERIF, color: C.bone, fontSize: 26, letterSpacing: 5, opacity: 0.75, position: 'absolute', right: 250, top: h / 2 - 16}}>
                {i === 4 ? '☄' : i === 6 ? '£' : ''}
              </div>
              {/* texture pixels */}
              {Array.from({length: 14}, (_, j) => (
                <div
                  key={j}
                  style={{
                    position: 'absolute',
                    left: random(`tx-${i}-${j}`) * W,
                    top: random(`ty-${i}-${j}`) * h,
                    width: 8 + random(`tw-${i}-${j}`) * 18,
                    height: 5,
                    background: 'rgba(0,0,0,0.35)',
                  }}
                />
              ))}
            </div>
          );
        })}
        {/* the shaft */}
        <div style={{position: 'absolute', left: W / 2 - 68, top: 0, width: 136, height: worldH, background: 'rgba(6,3,3,0.88)', borderLeft: `4px solid ${C.charcoal}`, borderRight: `4px solid ${C.charcoal}`}} />
      </div>
      {/* train nose-down, screen-fixed inside the shaft */}
      <div style={{position: 'absolute', left: W / 2 - 148, top: 200 + Math.sin(f / 6) * 6, transform: 'rotate(90deg)'}}>
        <HellTrain frame={f} u={4.6} glow={e} />
      </div>
      {/* flame trail above the train */}
      <svg width={W} height={H} style={{position: 'absolute', inset: 0}}>
        {Array.from({length: 8}, (_, i) => {
          const t = ((f * 3 + i * 14) % 110) / 110;
          return (
            <circle
              key={i}
              cx={W / 2 + (random(`ft-${i}`) - 0.5) * 60}
              cy={210 - t * 190}
              r={4 + t * 13}
              fill={t < 0.4 ? C.orange : C.charcoal}
              opacity={(1 - t) * 0.8}
            />
          );
        })}
      </svg>
      {/* gauges */}
      <div style={{position: 'absolute', right: 34, top: 90, fontFamily: SERIF, color: C.bone, fontSize: 30, textAlign: 'right', background: 'rgba(10,5,5,0.75)', border: `2px solid ${C.brass}`, padding: '12px 20px', letterSpacing: 2}}>
        DEPTH
        <br />
        <span style={{color: C.flame, fontSize: 44}}>−{depth} mi</span>
      </div>
      <div style={{position: 'absolute', left: 34, top: 90, fontFamily: SERIF, color: C.bone, fontSize: 30, background: 'rgba(10,5,5,0.75)', border: `2px solid ${C.brass}`, padding: '12px 20px', letterSpacing: 2}}>
        BOILER
        <br />
        <span style={{color: temp > 3000 ? C.red : C.flame, fontSize: 44}}>{temp.toLocaleString('en-GB')}°F</span>
      </div>
      <div style={{position: 'absolute', left: 0, right: 0, bottom: 16, textAlign: 'center', fontFamily: SERIF, color: C.gold, fontSize: 30, letterSpacing: 3, background: 'rgba(10,5,5,0.8)', padding: '8px 0'}}>
        CORING THE CRUST: {cheese.toFixed(0)}% <span style={{color: C.bone, fontSize: 22}}>(consistency: cheese)</span>
      </div>
      <StampWord from={Math.round(dur * 0.3)} word="DEEPER" size={140} />
      <StampWord from={Math.round(dur * 0.55)} word="HOTTER" size={140} color={C.ember} />
      <StampWord from={Math.round(dur * 0.9)} word="TUNNELS BY TUESDAY" size={80} color={C.flame} />
    </AbsoluteFill>
  );
};
