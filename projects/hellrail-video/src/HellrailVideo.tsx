import React from 'react';
import {AbsoluteFill, Audio, Sequence, staticFile, useCurrentFrame} from 'remotion';
import {energy} from './audio';
import {BoomScene, DescentScene, TitleScene} from './scenes1';
import {
  BlueprintScene,
  BoilerScene,
  EndCardScene,
  FinaleScene,
  FuelScene,
  LuxuryScene,
  VelocityScene,
  WheelsScene,
} from './scenes3';
import {LyricTrack} from './lyricTrack';
import {Aberrate, BeatFlashes, Camera, Patina} from './ui';

export const TOTAL_FRAMES = 6998;

type SceneDef = {
  from: number;
  dur: number;
  shake: number;
  aberrate: number;
  Comp: React.FC<{start: number; dur: number}>;
};

// Boundaries follow the sung stanzas of "HELLRAIL: Technical Specifications"
// (whisper word-level alignment in src/lyrics.json). Vocals start ~f950.
const SCENES: SceneDef[] = [
  {from: 0, dur: 430, shake: 0.7, aberrate: 0, Comp: TitleScene}, // instrumental
  {from: 430, dur: 520, shake: 1, aberrate: 0, Comp: BoomScene}, // instrumental overture
  {from: 950, dur: 333, shake: 0.5, aberrate: 0, Comp: BlueprintScene}, // st1: attend to the specs
  {from: 1283, dur: 707, shake: 1.6, aberrate: 0, Comp: BoilerScene}, // st2: firebox/boiler
  {from: 1990, dur: 755, shake: 2, aberrate: 2, Comp: WheelsScene}, // st3: wheels/axles
  {from: 2745, dur: 741, shake: 1.4, aberrate: 0, Comp: DescentScene}, // st4: drillbits/tunnels
  {from: 3486, dur: 726, shake: 0.6, aberrate: 0, Comp: LuxuryScene}, // st5: compartments
  {from: 4212, dur: 783, shake: 2.2, aberrate: 3, Comp: VelocityScene}, // st6: speed
  {from: 4995, dur: 730, shake: 1.5, aberrate: 2, Comp: FuelScene}, // st7: fuel
  {from: 5725, dur: 1120, shake: 1.3, aberrate: 0, Comp: FinaleScene}, // st8: subscribe!
  {from: 6845, dur: 153, shake: 0.2, aberrate: 0, Comp: EndCardScene},
];

export const HellrailVideo: React.FC = () => {
  const frame = useCurrentFrame();
  const e = energy(frame);
  // gentle global breathing zoom with the music
  const breathe = 1 + e * 0.012;
  return (
    <AbsoluteFill style={{background: '#000'}}>
      <Audio src={staticFile('hellrail_ad.wav')} />
      <AbsoluteFill style={{transform: `scale(${breathe})`}}>
        {SCENES.map(({from, dur, shake, aberrate, Comp}, i) => (
          <Sequence key={i} from={from} durationInFrames={dur}>
            <Camera frame={frame} shake={shake}>
              {aberrate > 0 ? (
                <Aberrate amount={aberrate * (0.4 + e)}>
                  <Comp start={from} dur={dur} />
                </Aberrate>
              ) : (
                <Comp start={from} dur={dur} />
              )}
            </Camera>
          </Sequence>
        ))}
      </AbsoluteFill>
      <LyricTrack descentRange={[2745, 3486]} />
      <BeatFlashes frame={frame} />
      <Patina frame={frame} />
    </AbsoluteFill>
  );
};
