import React from 'react';
import {AbsoluteFill, interpolate, random, useCurrentFrame} from 'remotion';
import {C, SERIF} from './theme';
import {energy, punch} from './audio';
import {
  Carriage,
  Crags,
  Demon,
  Flame,
  Gavel,
  HellTrain,
  MatchSeller,
  PentagramSeal,
  PieChart,
  RainLayer,
  Skyline,
  SmokeTrail,
  StockChart,
} from './art';
import {LyricCard, StampWord} from './ui';
import type {SceneProps} from './scenes1';

const W = 1280;
const H = 720;

// ---------------------------------------------------------------- 6. CRASH

const DEBTS = ['−£4,000', '−£12,660', '−£66,600', '−£8,214', '−£101,010', '−£3,333', '−£999,999', '−£17', '−£240,000', '−£66'];

export const CrashScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  const p = punch(start + f);
  const tilt = Math.sin(f / 5) * (1.2 + e * 2.2);
  const chartProgress = interpolate(f, [0, dur * 0.8], [0.4, 1], {extrapolateRight: 'clamp'});
  return (
    <AbsoluteFill style={{background: `linear-gradient(180deg, #0c0406 0%, ${C.demonDark} 70%, ${C.blood} 100%)`, transform: `rotate(${tilt}deg) scale(1.06)`}}>
      <Crags w={W} h={240} seed="cfar" color="#160608" y={H - 320} drift={f * 0.5} />
      {/* the chart dies in public */}
      <div style={{position: 'absolute', left: 90, top: 60, width: W - 180, height: 400, opacity: 0.85}}>
        <StockChart w={W - 180} h={400} progress={chartProgress} crashAt={0.5} color={C.red} />
        <div style={{position: 'absolute', left: 0, top: -6, fontFamily: SERIF, color: C.bone, fontSize: 22, letterSpacing: 3}}>
          HELLRAIL CONSOLIDATED — SHARE PRICE
        </div>
      </div>
      {/* the train derails, tumbling */}
      <div
        style={{
          position: 'absolute',
          left: 300 + Math.sin(f / 7) * 20,
          top: 380 + Math.min(140, f * 0.5),
          transform: `rotate(${Math.min(34, f * 0.35) + Math.sin(f / 4) * 3}deg)`,
        }}
      >
        <HellTrain frame={f * 2} u={7.6} glow={1} />
      </div>
      <SmokeTrail frame={f * 2} x={780} y={420} scale={1.6} n={12} />
      {/* sparks off the boiler */}
      <svg width={W} height={H} style={{position: 'absolute', inset: 0}}>
        {Array.from({length: 12}, (_, i) => {
          const t = ((f * 4 + i * 20) % 70) / 70;
          const a = random(`sp-${i}`) * Math.PI * 2;
          return (
            <rect
              key={i}
              x={620 + Math.cos(a) * t * 300}
              y={470 + Math.sin(a) * t * 200 + t * t * 120}
              width={5}
              height={5}
              fill={t < 0.5 ? C.yellow : C.ember}
              opacity={1 - t}
            />
          );
        })}
      </svg>
      {/* red ink rain + tumbling ledger pages */}
      <RainLayer frame={f} w={W} h={H} n={90} color={C.red} />
      {Array.from({length: 8}, (_, i) => {
        const x = random(`lp-${i}`) * W;
        const speed = 3 + random(`lps-${i}`) * 4;
        const y = ((random(`lpy-${i}`) * H + f * speed) % (H + 120)) - 80;
        return (
          <div
            key={i}
            style={{
              position: 'absolute',
              left: x + Math.sin(f / 10 + i) * 30,
              top: y,
              width: 54,
              height: 70,
              background: C.bone,
              transform: `rotate(${f * (2 + i)}deg)`,
              boxShadow: 'inset 0 12px 0 rgba(138,16,16,0.6), inset 0 -8px 0 rgba(0,0,0,0.15)',
            }}
          >
            {[18, 28, 38, 50].map((ly) => (
              <div key={ly} style={{position: 'absolute', left: 6, right: 6, top: ly, height: 3, background: i % 2 ? C.red : C.ash, opacity: 0.7}} />
            ))}
          </div>
        );
      })}
      {/* falling negative balances */}
      {DEBTS.map((d, i) => {
        const x = random(`db-${i}`) * (W - 200) + 60;
        const speed = 4 + random(`dbs-${i}`) * 5;
        const y = ((random(`dby-${i}`) * H + f * speed) % (H + 100)) - 60;
        return (
          <div key={i} style={{position: 'absolute', left: x, top: y, fontFamily: SERIF, fontWeight: 900, fontSize: 30 + random(`dbz-${i}`) * 26, color: C.red, textShadow: `0 0 12px ${C.blood}`, transform: `rotate(${(random(`dbr-${i}`) - 0.5) * 30}deg)`}}>
            {d}
          </div>
        );
      })}
      {/* a sweating banker in the corner */}
      <div style={{position: 'absolute', right: 60, bottom: 30, transform: `translateY(${Math.sin(f / 3) * 3}px)`}}>
        <Demon u={8.4} frame={f * 1.5} variant="banker" flip />
        <svg width={160} height={240} style={{position: 'absolute', inset: 0, overflow: 'visible'}}>
          {[0, 1, 2].map((i) => (
            <circle key={i} cx={30 + i * 24} cy={40 + ((f * 5 + i * 26) % 70)} r={4} fill="#60a0e0" opacity={0.9} />
          ))}
        </svg>
      </div>
      <LyricCard from={15} dur={110} text="THEN A TREMOR, THEN A CLANGOR" color={C.bone} />
      <LyricCard from={140} dur={115} text="A SULFUROUS SIDEWAYS LURCH" />
      <LyricCard from={275} dur={125} text="RED INK RAINED ON RUINED LEDGERS" color={C.red} />
      <LyricCard from={430} dur={105} text="CREDITORS GREW CLAMMY, CRAWLING IN SWEAT" size={38} />
      <LyricCard from={555} dur={105} text="SHAREHOLDERS IN SHEER HYSTERICS" />
      <LyricCard from={675} dur={150} text="SOLD AT SIXPENCE ON THE POUND" color={C.flame} size={52} y="48%" />
      <StampWord from={95} word="MARGIN CALL" size={110} color={C.red} />
      <StampWord from={375} word="PANIC" size={170} color={C.red} />
      <StampWord from={625} word="SELL! SELL! SELL!" size={92} color={C.red} />
      {/* full-screen red throb on the biggest hits */}
      <AbsoluteFill style={{background: C.red, opacity: p > 0.14 ? 0.25 : 0, pointerEvents: 'none'}} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- 7. COURT

const CHARGES = [
  'USURY IN THE FIRST DEGREE',
  'AGGRAVATED OPTIMISM',
  'FRAUD (SINCERE)',
  'IMPERSONATING A GOING CONCERN',
  'ARSON OF THE BALANCE SHEET',
  'CONSPIRACY TO COMPOUND',
  'EXCESSIVE MARZIPAN',
  'WATERING OF STOCK (1,000,000 GAL.)',
  'BANKRUPTCY IN PERPETUITY',
];

export const CourtScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  // gavel slams on a strict clock, like billable hours
  const cycle = 55;
  const cf = f % cycle;
  const gavelAngle = cf < 6 ? interpolate(cf, [0, 6], [-55, 8]) : cf < 10 ? interpolate(cf, [6, 10], [8, -8]) : interpolate(cf, [10, cycle], [-8, -55]);
  const slam = cf >= 5 && cf <= 8;
  const scrollY = (f * 1.4) % (CHARGES.length * 64 + 300);
  const interest = 6666 * Math.pow(1.02, f);
  return (
    <AbsoluteFill style={{background: `radial-gradient(ellipse at 50% 30%, #201018 0%, #0c0608 65%, ${C.ink} 100%)`}}>
      {/* stalactites */}
      <div style={{position: 'absolute', top: 0, left: 0, width: W, transform: 'scaleY(-1)'}}>
        <Crags w={W} h={130} seed="stal" color="#060304" y={0} />
      </div>
      {/* candelabra */}
      {[140, W - 140].map((x, i) => (
        <div key={i} style={{position: 'absolute', left: x - 40, top: 190}}>
          <div style={{position: 'absolute', left: 14, top: 60, width: 52, height: 12, background: C.brass}} />
          <div style={{position: 'absolute', left: 36, top: 72, width: 8, height: 90, background: C.brass}} />
          {[0, 30, 60].map((dx, j) => (
            <div key={j} style={{position: 'absolute', left: dx + 6, top: 26}}>
              <Flame frame={f} seed={`cnd-${i}-${j}`} u={4} />
            </div>
          ))}
        </div>
      ))}
      {/* the judge: an enormous silhouette with yellow eyes and a wig */}
      <div style={{position: 'absolute', left: W / 2 - 230, top: 40, width: 460, height: 420}}>
        <svg width={460} height={420} style={{overflow: 'visible'}}>
          <polygon points="30,420 90,120 370,120 430,420" fill="#08040a" />
          <circle cx={230} cy={110} r={85} fill="#08040a" />
          {/* wig */}
          <rect x={140} y={40} width={180} height={40} rx={20} fill="#d8d0c0" opacity={0.9} />
          <rect x={128} y={64} width={34} height={90} rx={17} fill="#d8d0c0" opacity={0.9} />
          <rect x={298} y={64} width={34} height={90} rx={17} fill="#d8d0c0" opacity={0.9} />
          {/* horns through the wig */}
          <polygon points="160,48 138,-8 178,28" fill={C.bone} />
          <polygon points="300,48 322,-8 282,28" fill={C.bone} />
          {/* eyes */}
          <rect x={186} y={100} width={26} height={10 + e * 8} fill={C.yellow} opacity={0.85 + e * 0.15} />
          <rect x={248} y={100} width={26} height={10 + e * 8} fill={C.yellow} opacity={0.85 + e * 0.15} />
        </svg>
      </div>
      {/* the bench */}
      <div style={{position: 'absolute', left: W / 2 - 300, top: 430, width: 600, height: 150, background: '#2a1810', borderTop: `10px solid #4a2c18`}} />
      <div style={{position: 'absolute', left: W / 2 - 300, top: 430, width: 600, textAlign: 'center', fontFamily: SERIF, color: C.gold, fontSize: 24, letterSpacing: 6, paddingTop: 16}}>
        COURT OF INFERNAL CHANCERY
      </div>
      {/* gavel + slam flash */}
      <div style={{position: 'absolute', left: W / 2 - 60, top: 350}}>
        <Gavel u={7} angle={gavelAngle} />
      </div>
      {slam ? (
        <>
          <div style={{position: 'absolute', left: W / 2 + 40, top: 470, width: 90, height: 90, border: `6px solid ${C.bone}`, borderRadius: '50%', opacity: 0.8, transform: `scale(${1 + (cf - 5) * 0.5})`}} />
          <AbsoluteFill style={{background: C.bone, opacity: 0.1}} />
        </>
      ) : null}
      {/* flanking barristers */}
      <div style={{position: 'absolute', left: 150, bottom: 20}}>
        <Demon u={9} frame={f} variant="barrister" />
      </div>
      <div style={{position: 'absolute', right: 150, bottom: 20}}>
        <Demon u={9} frame={f + 20} variant="barrister" flip />
      </div>
      {/* the scroll of charges */}
      <div style={{position: 'absolute', right: 30, top: 120, width: 300, height: 360, overflow: 'hidden', background: 'rgba(216,196,154,0.92)', border: `6px solid #8a6a38`, transform: 'rotate(2deg)'}}>
        <div style={{position: 'absolute', top: 60 - scrollY + 300, left: 0, right: 0}}>
          {[...CHARGES, ...CHARGES].map((c, i) => (
            <div key={i} style={{fontFamily: SERIF, color: C.soot, fontSize: 21, fontWeight: 700, letterSpacing: 1, padding: '18px 20px', borderBottom: `2px solid rgba(74,60,54,0.4)`}}>
              {i % CHARGES.length + 1}. {c}
            </div>
          ))}
        </div>
        <div style={{position: 'absolute', top: 0, left: 0, right: 0, background: '#8a6a38', color: C.bone, fontFamily: SERIF, fontSize: 20, letterSpacing: 4, textAlign: 'center', padding: '8px 0'}}>
          THE CHARGES
        </div>
      </div>
      <div style={{position: 'absolute', left: 0, right: 0, top: 10, textAlign: 'center', fontFamily: SERIF, color: C.gold, fontSize: 26, letterSpacing: 2, background: 'rgba(10,5,5,0.85)', padding: '6px 0'}}>
        INTEREST ON THE SOUL: {interest.toLocaleString('en-GB', {maximumFractionDigits: 0})}% AND CLIMBING
      </div>
      <LyricCard from={20} dur={125} text="DEMONS DRAFTED LEGAL DUNNING" y="88%" size={40} />
      <LyricCard from={165} dur={120} text="WRITS IN BRIMSTONE · BONDS IN BLOOD" y="88%" size={40} color={C.red} />
      <LyricCard from={310} dur={120} text="SOLICITORS IN SULFUR WIGS SERVED PAPERS" y="88%" size={38} />
      <LyricCard from={455} dur={120} text="COURT CONVENED IN CAVERNS CANDLEBLACK" y="88%" size={38} />
      <LyricCard from={595} dur={110} text="“BANKRUPTCY IN PERPETUITY," y="30%" size={46} color={C.flame} />
      <LyricCard from={710} dur={125} text="WITH INTEREST ON THE SOUL.”" y="30%" size={46} color={C.flame} />
      <StampWord from={250} word="OVERRULED" size={120} color={C.red} />
      <StampWord from={545} word="GUILTY-ISH" size={120} color={C.red} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- 8. RESTRUCTURE

const BUZZ = ['SYNERGY', 'ALIGNMENT', 'VALUE EXTRACTION', 'RIGHTSIZING', 'EBITDA (ETERNAL)', 'STAKEHOLDER SACRIFICE', 'CIRCLING BACK', 'LEVERAGE'];

export const RestructureScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const memoX = interpolate(f, [10, 45], [-620, 70], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const frac = interpolate(f, [120, dur - 160], [0.12, 0.00001], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const pct = frac * 100;
  const stamp = interpolate(f, [400, 406], [3, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  return (
    <AbsoluteFill style={{background: `linear-gradient(180deg, #b8b0a0 0%, #a09888 55%, #888070 100%)`}}>
      {/* fluorescent office light band — hell's true form */}
      <div style={{position: 'absolute', top: 0, left: 0, right: 0, height: 26, background: '#e8e4d8', boxShadow: '0 0 40px rgba(232,228,216,0.8)', opacity: 0.85 + Math.sin(f * 2.1) * 0.06}} />
      {/* hellscape visible through office window */}
      <div style={{position: 'absolute', right: 70, top: 70, width: 300, height: 200, border: '14px solid #706858', background: `linear-gradient(180deg, ${C.ink}, ${C.blood})`, overflow: 'hidden'}}>
        <Crags w={300} h={90} seed="win" color="#12080a" y={110} drift={f * 0.3} />
        <div style={{position: 'absolute', left: 20, top: 120, transform: 'scale(0.45)', transformOrigin: 'top left'}}>
          <HellTrain frame={f} u={4} glow={0.6} />
        </div>
        <div style={{position: 'absolute', bottom: 4, left: 0, right: 0, textAlign: 'center', fontFamily: SERIF, fontSize: 13, color: C.bone, letterSpacing: 2, opacity: 0.8}}>
          OPERATIONS (ONGOING)
        </div>
      </div>
      {/* buzzwords ascending like souls (behind the memo) */}
      {BUZZ.map((b, i) => {
        const y = H - (((f * (1.6 + random(`bz-${i}`) * 1.6) + i * 130) % (H + 140)) - 70);
        return (
          <div key={i} style={{position: 'absolute', left: 40 + random(`bx-${i}`) * (W - 400), top: y, fontFamily: SERIF, fontSize: 24 + random(`bs-${i}`) * 16, fontWeight: 700, color: 'rgba(36,26,20,0.4)', letterSpacing: 3, transform: `rotate(${(random(`br-${i}`) - 0.5) * 12}deg)`}}>
            {b}
          </div>
        );
      })}
      {/* THE MEMO */}
      <div style={{position: 'absolute', left: memoX, top: 60, width: 540, height: 560, background: '#f4eee0', boxShadow: '10px 14px 0 rgba(0,0,0,0.25)', transform: 'rotate(-1.2deg)', padding: 34, fontFamily: SERIF, color: '#241a14'}}>
        <div style={{fontSize: 24, letterSpacing: 3, borderBottom: '3px double #241a14', paddingBottom: 10, display: 'flex', alignItems: 'center', gap: 14}}>
          <PentagramSeal size={44} color={C.red} spin={f * 0.3} />
          PANDEMONIUM INTEROFFICE MEMORANDUM
        </div>
        <div style={{fontSize: 21, marginTop: 18, lineHeight: 1.65}}>
          <b>TO:</b> Shareholder №. 401,616 (you)
          <br />
          <b>FROM:</b> The Desk of the Larger Demon
          <br />
          <b>RE:</b> Your Investment (condolences)
        </div>
        <div style={{fontSize: 21, marginTop: 20, lineHeight: 1.7}}>
          Be advised the HELLRAIL has been{' '}
          <span style={{background: C.gold, padding: '0 6px', fontWeight: 900}}>ABSORBED</span> into{' '}
          <b>GEHENNA HOLDINGS</b> (“a family of brands”). Your shares have been diluted{' '}
          <i>for your convenience</i>. The Board expresses regret{' '}
          <span style={{fontSize: 15}}>(Form R-666, enclosed)</span> and thanks you for your venture.
        </div>
        {[0, 1, 2, 3].map((i) => (
          <div key={i} style={{height: 8, background: '#c8c0b0', marginTop: 14, width: `${88 - i * 14}%`}} />
        ))}
        {f > 400 ? (
          <div style={{position: 'absolute', bottom: 60, right: 30, transform: `rotate(-14deg) scale(${stamp})`, border: `6px solid ${C.red}`, color: C.red, fontSize: 40, fontWeight: 900, letterSpacing: 4, padding: '6px 18px', opacity: 0.85}}>
            RESTRUCTURED
          </div>
        ) : null}
      </div>
      {/* Gehenna Holdings corporate identity */}
      <div style={{position: 'absolute', right: 90, top: 320, width: 340, textAlign: 'center', fontFamily: SERIF}}>
        <div style={{display: 'inline-block', transform: `rotate(${Math.sin(f / 30) * 3}deg)`}}>
          <PentagramSeal size={130} color="#241a14" spin={f * 0.5} />
          <svg width={130} height={40} style={{marginTop: -20}}>
            <path d="M 5 20 Q 65 44 125 12" fill="none" stroke={C.red} strokeWidth={7} strokeLinecap="round" />
          </svg>
        </div>
        <div style={{fontSize: 34, letterSpacing: 5, fontWeight: 900, color: '#241a14'}}>GEHENNA HOLDINGS</div>
        <div style={{fontSize: 18, fontStyle: 'italic', color: '#443a30'}}>A Family of Brands™</div>
      </div>
      {/* your slice of the pie */}
      <div style={{position: 'absolute', right: 470, bottom: 36, textAlign: 'center'}}>
        <PieChart size={190} fraction={frac} />
        <div style={{fontFamily: SERIF, fontSize: 24, marginTop: 8, color: '#241a14', fontWeight: 700}}>
          YOUR SHARE: {pct < 0.01 ? pct.toFixed(5) : pct.toFixed(2)}%
        </div>
      </div>
      <LyricCard from={20} dur={120} text="A MEMO CAME FROM PANDEMONIUM" y="90%" size={40} color={C.bone} />
      <LyricCard from={160} dur={115} text="THE HELLRAIL HAD BEEN SOLD" y="90%" size={40} color={C.bone} />
      <LyricCard from={295} dur={140} text="ABSORBED BY A LARGER DEMON" y="90%" size={40} color={C.bone} />
      <LyricCard from={455} dur={135} text="MY SHARES WERE DILUTED DOWN TO NOTHING" y="90%" size={38} color={C.bone} />
      <LyricCard from={610} dur={110} text="THE BOARD EXPRESSED REGRET, OF COURSE" y="90%" size={38} color={C.bone} />
      <LyricCard from={725} dur={110} text="...AND THANKED ME FOR MY VENTURE" y="90%" size={38} color={C.flame} />
    </AbsoluteFill>
  );
};

// ---------------------------------------------------------------- 9. OUTRO

export const OutroScene: React.FC<SceneProps> = ({start, dur}) => {
  const f = useCurrentFrame();
  const e = energy(start + f);
  const trainT = interpolate(f, [320, 540], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const trainOn = f >= 320 && f <= 540;
  const worldFade = interpolate(f, [600, 680], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const endText = (at: number) =>
    ({
      opacity: interpolate(f, [at, at + 24], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'}),
    }) as React.CSSProperties;
  const endCard = interpolate(f, [858, 880], [0, 1], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  const finalFade = interpolate(f, [dur - 24, dur - 4], [1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
  return (
    <AbsoluteFill style={{background: C.ink}}>
      <AbsoluteFill style={{opacity: worldFade * finalFade, background: `linear-gradient(180deg, #0a0c14 0%, ${C.nightBlue} 55%, #10141c 100%)`}}>
        <div style={{position: 'absolute', left: 0, right: 0, bottom: 185, height: 230}}>
          <Skyline w={W} h={230} color="#080a10" seed="outro" />
        </div>
        {/* the train passes on the horizon, full of fresh investors */}
        {trainOn ? (
          <div style={{position: 'absolute', left: W + 100 - trainT * (W + 900), top: 388}}>
            <div style={{display: 'flex', alignItems: 'flex-end'}}>
              <div style={{transform: 'scaleX(-1)'}}>
                <HellTrain frame={f} u={3.6} glow={0.8} />
              </div>
              {[0, 1, 2].map((i) => (
                <div key={i} style={{marginLeft: -8}}>
                  <Carriage frame={f} u={3.6} seed={`out-${i}`} glow={0.9} />
                </div>
              ))}
            </div>
          </div>
        ) : null}
        {/* wet platform */}
        <div style={{position: 'absolute', left: 0, right: 0, bottom: 0, height: 200, background: '#0c0e16'}} />
        {Array.from({length: 12}, (_, i) => (
          <div key={i} style={{position: 'absolute', left: random(`refl-${i}`) * W, bottom: 20 + random(`refl2-${i}`) * 150, width: 2, height: 22 + random(`refl3-${i}`) * 26, background: C.grayBlue, opacity: 0.35}} />
        ))}
        {/* lamppost, dim */}
        <div style={{position: 'absolute', left: 1020, bottom: 180, width: 10, height: 260, background: '#04050a'}} />
        <div style={{position: 'absolute', left: 1004, bottom: 430, width: 42, height: 50, background: '#04050a'}} />
        <div style={{position: 'absolute', left: 1014, bottom: 442, width: 22, height: 26, background: C.gold, opacity: 0.25 + Math.sin(f / 7) * 0.06}} />
        {/* the narrator, selling matches */}
        <div style={{position: 'absolute', left: 300, bottom: 130, filter: 'saturate(0.35) brightness(0.8)'}}>
          <MatchSeller u={8.2} frame={f} />
        </div>
        {/* the match flame: the only warm thing left */}
        <div style={{position: 'absolute', left: 418, bottom: 318}}>
          <Flame frame={f} seed="match" u={3.4} />
        </div>
        <div style={{position: 'absolute', left: 402, bottom: 300, width: 60, height: 60, borderRadius: '50%', background: `radial-gradient(circle, rgba(248,184,48,0.3), transparent 70%)`}} />
        <RainLayer frame={f} w={W} h={H} n={80} />
      </AbsoluteFill>
      <LyricCard from={20} dur={150} text="NOW I STAND ON SOOTY SIDINGS, SELLING MATCHES IN THE RAIN" size={36} y="20%" />
      <LyricCard from={200} dur={130} text="AND SOMETIMES IN THE NIGHT I HEAR THE WHISTLE OF THE TRAIN..." size={34} y="20%" />
      <LyricCard from={430} dur={160} text="IT THUNDERS PAST WITH FRESH INVESTORS — CHAMPAGNE FLUTES RAISED HIGH AND NEW" size={32} y="20%" />
      {/* final lines on black */}
      <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center', fontFamily: SERIF, textAlign: 'center', opacity: finalFade}}>
        <div style={{fontSize: 46, color: C.bone, letterSpacing: 3, lineHeight: 2, textShadow: '0 3px 0 #000'}}>
          <div style={endText(640)}>BUT HELL HAS LOST MY FILE.</div>
          <div style={endText(715)}>I'M NOT WORTH DAMNING.</div>
          <div style={{...endText(790), color: C.flame, fontSize: 60, marginTop: 12}}>NOTHING'S DUE.</div>
        </div>
        <div style={{position: 'absolute', bottom: 46, opacity: endCard, fontSize: 17, color: C.ash, letterSpacing: 2, lineHeight: 1.8}}>
          THE HELLRAIL — TERMS APPLY IN PERPETUITY
          <br />
          a paid promotion of GEHENNA HOLDINGS · past performance is a guarantee of future results
        </div>
        <div style={{position: 'absolute', bottom: 130, opacity: endCard * 0.7}}>
          <PentagramSeal size={54} color={C.blood} spin={f * 0.2} />
        </div>
      </AbsoluteFill>
    </AbsoluteFill>
  );
};
