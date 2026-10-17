// Extracts a per-video-frame RMS loudness envelope from the WAV so the
// composition can be audio-reactive without decoding audio at render time.
import {readFileSync, writeFileSync} from 'node:fs';
import {join, dirname} from 'node:path';
import {fileURLToPath} from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const buf = readFileSync(join(root, 'public', 'hellrail_ad.wav'));

// Parse RIFF chunks to find fmt + data
let pos = 12;
let fmt = null;
let dataOff = -1;
let dataLen = 0;
while (pos + 8 <= buf.length) {
  const id = buf.toString('ascii', pos, pos + 4);
  const size = buf.readUInt32LE(pos + 4);
  if (id === 'fmt ') {
    fmt = {
      channels: buf.readUInt16LE(pos + 10),
      sampleRate: buf.readUInt32LE(pos + 12),
      bitsPerSample: buf.readUInt16LE(pos + 22),
    };
  } else if (id === 'data') {
    dataOff = pos + 8;
    dataLen = size;
  }
  pos += 8 + size + (size % 2);
}
if (!fmt || dataOff < 0) throw new Error('bad wav');
console.log(fmt);

const bytesPerSample = fmt.bitsPerSample / 8;
const frameCount = dataLen / (bytesPerSample * fmt.channels);
const fps = 30;
const perFrame = fmt.sampleRate / fps;
const videoFrames = Math.ceil(frameCount / perFrame);

const env = [];
for (let v = 0; v < videoFrames; v++) {
  const start = Math.floor(v * perFrame);
  const end = Math.min(frameCount, Math.floor((v + 1) * perFrame));
  let sum = 0;
  let n = 0;
  for (let s = start; s < end; s += 4) {
    // sample a subset for speed; average channels
    for (let c = 0; c < fmt.channels; c++) {
      const off = dataOff + (s * fmt.channels + c) * bytesPerSample;
      const val = bytesPerSample === 2 ? buf.readInt16LE(off) / 32768 : (buf.readUInt8(off) - 128) / 128;
      sum += val * val;
      n++;
    }
  }
  env.push(Math.sqrt(sum / Math.max(1, n)));
}
// Normalize 0..1 against 98th percentile to resist outliers
const sorted = [...env].sort((a, b) => a - b);
const p98 = sorted[Math.floor(sorted.length * 0.98)] || 1;
const norm = env.map((e) => Math.min(1, e / p98));
writeFileSync(join(root, 'src', 'envelope.json'), JSON.stringify(norm.map((e) => Math.round(e * 1000) / 1000)));
console.log('frames:', norm.length);
