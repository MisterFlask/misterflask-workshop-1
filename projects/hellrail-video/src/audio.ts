import envelope from './envelope.json';

const E = envelope as number[];

/** Raw per-frame RMS loudness, 0..1, indexed by ABSOLUTE frame. */
export const rawEnergy = (f: number): number =>
  E[Math.min(E.length - 1, Math.max(0, Math.round(f)))] ?? 0;

/** Smoothed loudness over a +/- w frame window. */
export const energy = (f: number, w = 4): number => {
  let s = 0;
  let n = 0;
  for (let i = -w; i <= w; i++) {
    s += rawEnergy(f + i);
    n++;
  }
  return s / n;
};

/** Transient punch: how much louder this frame is than the recent past. */
export const punch = (f: number): number => {
  const past = (rawEnergy(f - 3) + rawEnergy(f - 6) + rawEnergy(f - 9)) / 3;
  return Math.max(0, rawEnergy(f) - past);
};

/**
 * Pre-scanned loud transients (absolute frames), spaced apart — used for
 * single-frame flash cuts so the strobes land on actual audio hits.
 */
export const transients: number[] = (() => {
  const out: number[] = [];
  for (let f = 90; f < E.length - 30; f++) {
    const past = (E[f - 3] + E[f - 6] + E[f - 9]) / 3;
    const p = E[f] - past;
    if (p > 0.16 && (out.length === 0 || f - out[out.length - 1] > 110)) {
      out.push(f);
    }
  }
  return out;
})();

/** Returns the index of the transient hitting at frame f (within 2 frames), or -1. */
export const transientAt = (f: number): number => {
  for (let i = 0; i < transients.length; i++) {
    const d = f - transients[i];
    if (d >= 0 && d <= 2) return i;
    if (transients[i] > f) return -1;
  }
  return -1;
};
