export const meta = {
  name: 'adjudicate-blind-clip',
  description: 'Judge each validator failure on blind-authored unit clips: blind viewers, metric auditors, per-failure adjudicators',
  phases: [
    { title: 'View', detail: '3 blind viewers list every defect they see, without the validator verdicts' },
    { title: 'Audit', detail: 'one metric auditor per clip reads the metric code against the clip' },
    { title: 'Adjudicate', detail: 'one adjudicator per failure: real defect, subtle, false positive or metadata' },
  ],
}

// Claude Code Workflow script (docs/ANIMATION.md section 7): adjudicates every hard failure the
// validator reports on a clip authored without it. args: {dir (evidence: blind/y<yaw>/ sheets from
// `r3d-anim.mjs sheet --blind`, evidence/front|side/ sheets with verdicts, focus images), recipe,
// unit (a phrase for the prompts), clipFile, failures: [{clip, id, value, limit, frame, note,
// focus}]}. Blind viewers list defects without the verdicts; an auditor per clip reads each
// metric's code against the clip; an adjudicator per failure argues both sides and decides.

const D = args.dir
const REPO = args.repo ?? '/home/user/misterflask-workshop-1/projects/ffh2-rs'
const CLIPS = ['idle', 'attack', 'death']
const RECIPE = args.recipe
const blindSheets = (c) => [0.25, 1.8, 3.4].map((y) => `${D}/blind/y${y}/${RECIPE}-${c}-human.png`)

const CONTEXT = `Context: a fantasy strategy game renders units as small 3D figures seen from above at about 50 degrees; at gameplay zoom a figure is about 30 px tall, at the closest zoom about 70 px. Each unit has an idle loop, an attack and a death clip, played on the GPU by lerping baked frames (a one-shot cross-fades in from the idle over 0.15 s). The figure here is ${args.unit}. A review sheet shows sampled frames at gameplay size (top strip), the same frames enlarged in cells labelled with the time in seconds (the cells sample the clip: judge speed from the times), a timing chart (onion skin with one dot per frame on the striking point in red, the head in blue, the pelvis in green: dot spacing = speed) and speed/height curves over time.`

const VIEW_SCHEMA = {
  type: 'object',
  properties: {
    clips: { type: 'array', items: { type: 'object', properties: {
      clip: { type: 'string', enum: CLIPS },
      overall: { type: 'string', description: 'two or three sentences: how good is this as game animation' },
      defects: { type: 'array', items: { type: 'object', properties: {
        frames: { type: 'string', description: 'the cells, by their times, e.g. "0.40-0.47 s"' },
        view: { type: 'string', description: 'which sheet(s): default, side, behind' },
        what: { type: 'string' },
        severity: { type: 'string', enum: ['major', 'minor', 'nitpick'] },
      }, required: ['frames', 'view', 'what', 'severity'] } },
    }, required: ['clip', 'overall', 'defects'] } },
  },
  required: ['clips'],
}

const AUDIT_SCHEMA = {
  type: 'object',
  properties: {
    items: { type: 'array', items: { type: 'object', properties: {
      id: { type: 'string' },
      measures: { type: 'string', description: 'what the metric actually computes, from the code' },
      measured_right: { type: 'boolean', description: 'is the reported value a correct measurement of what the metric claims, for this clip (no artefact of detection, units, definitions)?' },
      defect_in_clip: { type: 'string', enum: ['yes', 'no', 'unclear'], description: 'does the clip really have the defect the metric names, at the reported frame(s)?' },
      analysis: { type: 'string', description: 'the evidence: code paths, clip keys, numbers, what the images show' },
      remedy: { type: 'string', enum: ['fix the clip', 'fix the metric', 'both', 'neither'] },
      remedy_detail: { type: 'string' },
    }, required: ['id', 'measures', 'measured_right', 'defect_in_clip', 'analysis', 'remedy', 'remedy_detail'] } },
  },
  required: ['items'],
}

const VERDICT_SCHEMA = {
  type: 'object',
  properties: {
    verdict: { type: 'string', enum: ['real-visible', 'real-subtle', 'false-positive', 'metadata'] },
    confidence: { type: 'string', enum: ['low', 'medium', 'high'] },
    viewer_corroborated: { type: 'boolean', description: 'did at least one blind viewer independently report this defect (same kind, nearby frames)?' },
    case_for_false_positive: { type: 'string' },
    case_for_real: { type: 'string' },
    reasoning: { type: 'string', description: 'why the winning case wins' },
    remedy: { type: 'string', enum: ['fix the clip', 'fix the metric', 'both', 'neither'] },
    remedy_detail: { type: 'string' },
  },
  required: ['verdict', 'confidence', 'viewer_corroborated', 'case_for_false_positive', 'case_for_real', 'reasoning', 'remedy', 'remedy_detail'],
}

phase('View')
const viewers = parallel([0, 1, 2].map((j) => () => agent(`${CONTEXT}

You are an experienced game animator reviewing three clips of this unit. Look at every image below with the Read tool (each clip from the default view, the side and behind) and list every animation defect you can see (cite cells by their time): physical errors (sliding or floating feet, parts through the body or the ground, broken joints, stretching), timing and spacing problems (floaty, mushy, no anticipation, pops, jerks, hitches), weight, readability at 30 px, staging, appeal. Give frame indices from the cell labels. Be specific and honest: report what you see, not what you would expect; an empty defect list is fine for a clip that is good. Do not read any code or other files; judge only the images.

${CLIPS.map((c) => `- ${c}: ${blindSheets(c).join(' ; ')}`).join('\n')}`, { label: `viewer:${j + 1}`, phase: 'View', schema: VIEW_SCHEMA })))

phase('Audit')
const byClip = {}
for (const f of args.failures) (byClip[f.clip] ??= []).push(f)
const audits = parallel(CLIPS.filter((c) => byClip[c]).map((c) => () => agent(`${CONTEXT}

You are auditing an automated animation validator. Its source is ${REPO}/crates/ffh-play/r3d-lab/anim/check.mjs (LIMITS at the top explain every threshold; the metric ids map to functions: ground/feet/rigid/joints/clear/balance groups, playback(), continuity() and tracks() for pop.*, transition() for boundary.transition, idle(), attackKeys()/attack() for attack.*, the reading checks for read.*, death() for death.*). The clip engine is ${REPO}/crates/ffh-play/assets/app/r3d/anim.js; the figure generator ${REPO}/crates/ffh-play/assets/app/r3d/assets/units.js. The clip under test was authored by an animator who never saw the validator: ${args.clipFile} (clip "${c}"). Do not edit any file.

The validator reports these hard failures on the ${c} clip (human figure):
${byClip[c].map((f) => `- ${f.id}: value ${JSON.stringify(f.value)}, limit ${JSON.stringify(f.limit)}, worst frame ${f.frame ?? 'n/a'} — "${f.note}". Close-up of the frames around it (default view, side, behind, and gameplay size): ${f.focus}`).join('\n')}
Full review sheets with the validator's verdicts: ${D}/evidence/front/${RECIPE}-${c}-human.png and ${D}/evidence/side/${RECIPE}-${c}-human.png.

For each failure: read the metric's code and decide (1) whether the value is a correct measurement of what the metric claims, for this clip (watch for detection artefacts: onset thresholds, which vertices count as trunk, definitions that assume a chop rather than a thrust, events that mean something else to the author), and (2) whether the clip really has the defect the metric names. You may write and run small node scripts in ${D}/audit-${c}/ to inspect the baked frames (import bakeClip from anim.js after importing assets/index.js and assets/anims.js; the registered clip is the same as ${args.clipFile}). Look at the close-ups. Then say whether the clip or the metric should change.`, { label: `audit:${c}`, phase: 'Audit', schema: AUDIT_SCHEMA })))

const [V, Au] = await Promise.all([viewers, audits])
const auditOf = {}
for (const a of Au.filter(Boolean)) for (const it of a.items ?? []) auditOf[it.id + '@'] = auditOf[it.id + '@'] ?? []
const auditFor = (f) => {
  const ci = CLIPS.filter((c) => byClip[c]).indexOf(f.clip)
  return (Au[ci]?.items ?? []).find((it) => it.id === f.id) ?? null
}
const viewsFor = (c) => V.filter(Boolean).map((v, i) => {
  const cl = (v.clips ?? []).find((x) => x.clip === c)
  return cl ? `Viewer ${i + 1}: overall "${cl.overall}"; defects: ${cl.defects.map((d) => `[${d.severity}] ${d.frames} (${d.view}): ${d.what}`).join(' | ') || 'none'}` : `Viewer ${i + 1}: no answer`
}).join('\n')

phase('Adjudicate')
const verdicts = parallel(args.failures.map((f) => () => {
  const a = auditFor(f)
  return agent(`${CONTEXT}

An automated animation validator flagged a hard failure on a clip authored by an animator who never saw the validator. Your job is to decide, adversarially, whether the validator is right. This decides whether the validator's false-positive rate is acceptable, so be strict both ways: do not defend the validator, do not defend the clip.

The failure: clip "${f.clip}", metric ${f.id}, value ${JSON.stringify(f.value)}, limit ${JSON.stringify(f.limit)}, worst frame ${f.frame ?? 'n/a'} — "${f.note}".
Close-up of the frames around it (default view, side, behind, gameplay size): ${f.focus}
Full sheets with the validator's verdicts: ${D}/evidence/front/${RECIPE}-${f.clip}-human.png, ${D}/evidence/side/${RECIPE}-${f.clip}-human.png
Metric code: ${REPO}/crates/ffh-play/r3d-lab/anim/check.mjs (LIMITS at the top give each threshold's reason). Clip source: ${args.clipFile}. Do not edit any file.

A metric auditor's analysis: ${a ? JSON.stringify(a) : '(none)'}

Three animators reviewed the clip's sheets blind (they did not know what the validator flagged):
${viewsFor(f.clip)}

Look at the images yourself (Read tool) and check the auditor's claims against the code where they matter. Make the strongest case that this is a false positive (the clip is fine there and the metric misjudges it: a wrong definition, a threshold too strict for valid animation, a measurement artefact), then the strongest case that it is real, then decide:
- real-visible: a defect a player or a reviewer would see (at gameplay or the closest zoom);
- real-subtle: physically or in craft wrong, seen only on close inspection, legitimately flagged;
- false-positive: the metric is wrong about this clip;
- metadata: declared metadata disagrees with the motion; not a visual defect.`, { label: `adjudicate:${f.clip}:${f.id}`, phase: 'Adjudicate', schema: VERDICT_SCHEMA }).then((v) => ({ ...f, audit: a, ...(v ?? { verdict: 'none' }) }))
}))
const R = await verdicts
const count = (k) => R.filter((r) => r.verdict === k).length
return {
  verdicts: R.map((r) => ({ clip: r.clip, id: r.id, verdict: r.verdict, confidence: r.confidence, corroborated: r.viewer_corroborated, remedy: r.remedy, remedy_detail: r.remedy_detail, reasoning: r.reasoning, case_for_false_positive: r.case_for_false_positive, case_for_real: r.case_for_real, audit: r.audit && { measured_right: r.audit.measured_right, defect_in_clip: r.audit.defect_in_clip, analysis: r.audit.analysis, remedy: r.audit.remedy, remedy_detail: r.audit.remedy_detail } })),
  viewers: V,
  summary: { total: R.length, realVisible: count('real-visible'), realSubtle: count('real-subtle'), falsePositive: count('false-positive'), metadata: count('metadata') },
}