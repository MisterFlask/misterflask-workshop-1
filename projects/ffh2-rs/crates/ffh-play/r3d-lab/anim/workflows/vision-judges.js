export const meta = {
  name: 'anim-vision-judges',
  description: 'Blind vision judges for R3D unit animations: recognition and good-vs-mutant forced choice',
  phases: [
    { title: 'Recognize', detail: '4 judges name the action, direction, end pose and key moment of each clip' },
    { title: 'Compare', detail: '10 judges pick the better of two versions of a clip (a degraded copy or an earlier draft), both orders' },
  ],
}

// Claude Code Workflow script (docs/ANIMATION.md section 6): blind vision judges over a pack from
// `node tools/r3d-anim.mjs judge-pack --out DIR [--before FILE,..]`. args: {dir (DIR/stimuli),
// questions, key} from the pack (file names relative to dir). The key scores the answers only
// after every judge has answered; no prompt names a clip, a mutation or a verdict.

// args: {questions: {recognize: [{id, files}], pairs: [{id, group, labels, files}]}, key: {...}}
// The key is used only below, to score; no prompt contains it.
const Q = args.questions, KEY = args.key, DIR = args.dir

const RECOGNIZE_SCHEMA = {
  type: 'object',
  properties: {
    items: { type: 'array', items: { type: 'object', properties: {
      id: { type: 'string' },
      description: { type: 'string', description: 'what you see happen, in one or two sentences' },
      action: { type: 'string', enum: ['standing idle', 'melee attack', 'shooting a bow', 'dying', 'walking', 'casting', 'cannot tell'] },
      direction: { type: 'string', enum: ['left', 'right', 'up (away from the viewer)', 'down (toward the viewer)', 'none', 'cannot tell'] },
      final: { type: 'string', enum: ['standing', 'crouching', 'kneeling', 'sitting', 'lying', 'cannot tell'] },
      moment: { type: 'integer', description: 'the numbered cell of the main moment (the blow landing, the arrow loosed, the body hitting the ground); 0 if none' },
      flaws: { type: 'string', description: 'anything that looks wrong or unnatural as animation, or "none"' },
    }, required: ['id', 'description', 'action', 'direction', 'final', 'moment', 'flaws'] } },
  },
  required: ['items'],
}

const PAIR_SCHEMA = {
  type: 'object',
  properties: {
    items: { type: 'array', items: { type: 'object', properties: {
      id: { type: 'string' },
      choice: { type: 'string', description: 'the token of the better game animation' },
      reason: { type: 'string', description: 'one sentence' },
      confidence: { type: 'string', enum: ['low', 'medium', 'high'] },
    }, required: ['id', 'choice', 'reason', 'confidence'] } },
  },
  required: ['items'],
}

const SHEET = `Each image is a review sheet of one small unit from a fantasy strategy game, seen from above and at an angle, as players see it. The top strip shows sampled frames left to right at the true size players see (about 30 pixels tall); below are the same frames enlarged 2-3x in cells labelled with a number and the time in seconds (the cells sample the clip: read the times, not the cell count, to judge speed), read left to right and top to bottom; at the bottom left an onion-skin with a dot per frame on three moving points (red, blue, green: the dots' spacing shows speed, close dots = slow), and at the bottom right curves of speed (red) and height (blue, green) over time. Look at the images with the Read tool before answering.`

// Deterministic shuffles (no Math.random in workflows).
function shuffled(arr, seed) {
  const a = arr.slice()
  let s = seed >>> 0
  for (let i = a.length - 1; i > 0; i--) {
    s = (Math.imul(s ^ (s >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) >>> 0
    const j = s % (i + 1);
    [a[i], a[j]] = [a[j], a[i]]
  }
  return a
}

phase('Recognize')
const recog = parallel([0, 1, 2, 3].map((j) => () => {
  const items = shuffled(Q.recognize, 1000 + j * 7919)
  const list = items.map((it) => `- item ${it.id}: ${DIR}/${it.files[0]}`).join('\n')
  return agent(`You are judging animations blind. ${SHEET}

For each item below, look at its image and answer: what the unit is doing (action), toward which edge of the image any action is directed (direction; for a fall, which way the body falls), its pose in the last frame (final), the numbered cell of the main moment (0 if none), and any flaws you see as animation. Judge only what you see; do not guess from file names. Answer every item.

${list}`, { label: `recognize:${j + 1}`, phase: 'Recognize', schema: RECOGNIZE_SCHEMA })
}))

phase('Compare')
// Pair judges: judge j sees, for each clip, one pair group (by index) in one order, so no judge
// sees the same good clip twice (which would let it spot the common one); with 10 judges every
// group of a clip with up to 5 groups is seen in both orders.
const groups = {}
for (const p of Q.pairs) {
  const clip = p.group.split('/').slice(0, 2).join('/')
  ;(groups[clip] ??= []).push(p)
}
const compare = parallel([0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map((j) => () => {
  const mine = []
  for (const [clip, ps] of Object.entries(groups)) {
    const muts = [...new Set(ps.map((p) => p.group))]
    const g = muts[j % muts.length]
    const order = Math.floor(j / muts.length) % 2
    const cands = ps.filter((p) => p.group === g)
    mine.push(cands[order % cands.length])
  }
  const list = shuffled(mine, 77 + j * 104729).map((p) => `- item ${p.id}: ${p.labels[0]} = ${DIR}/${p.files[0]} ; ${p.labels[1]} = ${DIR}/${p.files[1]}`).join('\n')
  return agent(`You are judging animations blind. ${SHEET}

Each item below gives two images of the same unit doing the same thing, each named by a token. For each item, look at both images and choose the better game animation (more readable at the size players see it, more natural weight and timing, fewer flaws). Answer with the token, one sentence why, and your confidence. Answer every item.

${list}`, { label: `compare:${j + 1}`, phase: 'Compare', schema: PAIR_SCHEMA })
}))

const [R, P] = await Promise.all([recog, compare])

// --- scoring (the key is used here only) ---
const rec = {}
for (const r of R.filter(Boolean)) for (const a of r.items ?? []) (rec[a.id] ??= []).push(a)
const recognition = Object.entries(KEY.recognize).map(([id, k]) => {
  const as = rec[id] ?? []
  const ok = (f) => as.filter(f).length
  return {
    clip: `${k.recipe} ${k.clip} (${k.view})`,
    judges: as.length,
    action: `${ok((a) => a.action === k.action)}/${as.length}`,
    direction: k.direction ? `${ok((a) => a.direction === k.direction)}/${as.length}` : 'n/a',
    final: `${ok((a) => a.final === k.final)}/${as.length}`,
    moment: k.moment ? `${ok((a) => Math.abs(a.moment - k.moment) <= 1)}/${as.length} within 1 cell of ${k.moment}` : 'n/a',
    answers: as.map((a) => `${a.action} | ${a.direction} | ${a.final} | cell ${a.moment} | ${a.description} | flaws: ${a.flaws}`),
  }
})
const pairs = {}
let first = 0, total = 0
for (const r of P.filter(Boolean)) for (const a of r.items ?? []) {
  const k = KEY.pairs[a.id]
  if (!k) continue
  const q = Q.pairs.find((p) => p.id === a.id)
  total++
  if (a.choice === q.labels[0]) first++
  const g = `${k.recipe} ${k.clip} vs ${k.mutant}`
  ;(pairs[g] ??= []).push({ order: k.order, correct: a.choice === k.good, confidence: a.confidence, reason: a.reason })
}
const comparison = Object.entries(pairs).map(([g, vs]) => ({ pair: g, correct: `${vs.filter((v) => v.correct).length}/${vs.length}`, votes: vs }))
const allPairs = Object.values(pairs).flat()
return {
  recognition,
  comparison,
  summary: {
    pairAccuracy: `${allPairs.filter((v) => v.correct).length}/${allPairs.length}`,
    positionBias: `${first}/${total} chose the first image`,
  },
}
