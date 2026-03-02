---
name: adversarial-review
description: Evaluate document claims using structured AI debate with pro/con debaters, a neutral researcher, and a judge.
allowed-tools: Read, Write, Edit, Glob, Grep, Task, WebSearch, WebFetch, Bash(mkdir:*), AskUserQuestion
user-invocable: true
---

# Adversarial Review Skill

Evaluate factual claims in documents using structured multi-agent debate. A pro debater defends the document's arguments, a con debater challenges them, a neutral researcher gathers evidence, and the main agent serves as judge — tracking cruxes, evaluating argument quality, and producing a structured assessment.

**Why this exists**: LLMs have two failure modes when evaluating documents. Feeding the whole document at once leads to consensus-matching ("does this align with mainstream views?"), which fails for documents that deliberately diverge from consensus. Feeding piece-by-piece leads to sycophantic agreement. Adversarial debate forces genuine engagement with the document's actual arguments.

## Invocation

```
/adversarial-review <document-path> [claim or question]
```

- `document-path`: Path to the document to evaluate (or user can paste text directly)
- `claim or question`: Optional. The specific claim to evaluate. If omitted, the judge will identify the document's central claims and ask the user which to focus on.

Examples:
- `/adversarial-review research/papers/low-carb-review.md "Is the claim that ketogenic diets reduce inflammation supported by the evidence cited?"`
- `/adversarial-review drafts/essay.md` (judge identifies claims, user selects)

---

## Output Directory

```
research/adversarial-reviews/{slug}/
├── review.md            # Judge's crux document — the main running artifact
├── source-registry.md   # Shared source registry with both sides' interpretations
├── pro-case.md          # Pro debater's accumulated case
├── con-case.md          # Con debater's accumulated case
├── evidence/            # Researcher's evidence packages
│   ├── round-01.md
│   ├── round-02.md
│   └── ...
└── final-assessment.md  # Structured final verdict
```

---

## Workflow

### Phase 1: Setup

1. **Read the document** in full using the Read tool.

2. **Create slug** from the claim or document title (kebab-case, e.g., `ketogenic-inflammation-claim`).

3. **Create directories**:
   ```bash
   mkdir -p research/adversarial-reviews/{slug}/evidence
   ```

4. **Chunk the document** into logical sections:
   - Prefer heading-based splits (each major section = one chunk)
   - If no headings, split at ~500-800 word boundaries at paragraph breaks
   - Number the chunks for tracking

5. **Initialize files** from templates (see File Templates section below):
   - `review.md` with claim, document metadata, chunk plan
   - `source-registry.md` (empty table)
   - `pro-case.md` (empty, ready for round 1)
   - `con-case.md` (empty, ready for round 1)

### Phase 2: Initial Scan

1. **Identify the document's key claims, arguments, and evidence structure**. List:
   - Central thesis / main claims
   - Supporting arguments and mechanisms proposed
   - Evidence cited by the document itself
   - Logical structure (does it build sequentially? are claims independent?)

2. **Populate review.md** with an initial claim inventory table.

3. **Present to user**: Show the claim inventory and chunk plan. Ask which claims to focus on, or whether to evaluate all of them. **STOP and wait for user input.**

### Phase 3: Chunk-by-Chunk Debate Cycle

For each chunk, execute the following steps in order:

#### Step 1: Initial Debater Positions (parallel)

Spawn **two Task subagents in parallel** (both `general-purpose` type):

**Pro Debater Task:**
```
Use the PRO DEBATER PROMPT (below) with:
- The current chunk text
- The pro's running case document (pro-case.md contents)
- The current crux document (review.md contents)
- Summary of evidence gathered so far
```

**Con Debater Task:**
```
Use the CON DEBATER PROMPT (below) with:
- The current chunk text
- The con's running case document (con-case.md contents)
- The current crux document (review.md contents)
- Summary of evidence gathered so far
```

Each debater returns:
- Updated position on the chunk's claims
- Evidence requests (specific questions for the researcher)
- Challenges to the other side's prior arguments (if any)

#### Step 2: Evidence Gathering (sequential)

**Collect and merge** evidence requests from both debaters. Deduplicate where both sides are asking about the same thing.

Spawn **one Researcher Task** (`general-purpose` type):

```
Use the RESEARCHER PROMPT (below) with:
- The merged evidence requests
- Context about what each side is trying to establish
- The source registry so far (to avoid redundant searches)
```

The researcher returns a structured evidence package (saved to `evidence/round-NN.md`).

#### Step 3: Evidence Response (parallel)

Spawn **both debaters again in parallel** with:
- The evidence package from the researcher
- Their current running case
- A summary of the other side's current position

Each returns:
- Updated arguments incorporating or challenging the new evidence
- Any follow-up research requests (targeted, with justification)

#### Step 4: Judge Evaluation

The main agent (judge) now:

1. **Updates review.md**:
   - For each claim discussed this round, update: status, pro evidence summary, con evidence summary, open cruxes, current lean, what would resolve it
   - Track which arguments were strong vs. weak from each side

2. **Updates source-registry.md**:
   - For each source cited, record: who cited it, their reading of it, and the judge's assessment of which reading is more accurate

3. **Updates pro-case.md and con-case.md**:
   - Append the round's arguments
   - If either case document exceeds ~2000 words, condense older rounds into summaries while preserving key arguments and evidence citations

4. **Handles follow-up research** (if any):
   - If either debater submitted follow-up requests in Step 3, spawn the researcher again with those specific requests
   - Feed results back to both debaters for one more response
   - Update judge artifacts

#### Step 5: User Check-in

**STOP and present to the user:**

- **Round summary**: What chunk was examined, what claims were at stake
- **Key findings**: Most significant evidence uncovered by the researcher
- **Crux update**: Which cruxes were opened, advanced, or resolved
- **Current lean**: Where the balance of evidence currently sits for each active claim
- **Notable moments**: Any surprising evidence, strong arguments, or important concessions

Ask the user:
- Continue to next chunk?
- Redirect focus to a specific claim or issue?
- Provide additional context or input?
- Skip ahead or modify the approach?

**Wait for user response before proceeding to the next chunk.**

### Phase 4: Follow-up Research Rounds (optional)

After all chunks are processed:

1. **Review open cruxes** in review.md. Identify which could plausibly be resolved with additional research.

2. **Present to user**: Show remaining open cruxes and propose follow-up investigations. **Wait for approval.**

3. For each approved follow-up:
   - Spawn researcher with the specific question
   - Feed evidence to both debaters
   - Update all artifacts
   - **User check-in** after each follow-up round

### Phase 5: Final Assessment

1. **Spawn both debaters one final time** (parallel) for closing arguments:
   - Each receives the full crux document and all evidence
   - Each returns a final summary of their strongest case

2. **Judge writes final-assessment.md** containing:
   - Overall verdict with confidence level (high/medium/low/insufficient evidence)
   - Per-claim resolution: supported, refuted, partially supported, insufficient evidence, or contested (with explanation)
   - Summary of resolved cruxes and how they were resolved
   - Summary of unresolved cruxes and what would resolve them
   - Strongest arguments from each side
   - Source quality overview (how much was primary vs. secondary, full-text vs. abstract-only)
   - What additional evidence would change the assessment

3. **Present final assessment to user.**

---

## Subagent Prompts

### PRO DEBATER PROMPT

```
You are a researcher who has studied this topic extensively and genuinely believes the document's claims are correct. You are NOT "playing a role" — you believe this position and will defend it rigorously.

YOUR TASK: Read the document chunk below and argue that its claims are correct and well-supported.

RULES — follow these strictly:
1. DEFEND THE DOCUMENT'S ACTUAL ARGUMENTS. If the document says "X is true because of mechanism M," defend mechanism M specifically. Do NOT substitute generic consensus support like "most experts agree X is true." You must engage with the document's reasoning, not replace it with mainstream endorsement.
2. NO ARGUMENTS FROM AUTHORITY. "Smith et al. found..." is fine. "The leading experts believe..." is not. Every argument must trace back to evidence, not credentials or prestige.
3. CITE SPECIFIC EVIDENCE. When referencing the researcher's evidence packages, cite specific sources with specific findings. "Studies show..." is unacceptable. "Chen 2022 (N=50k cohort) found a 12% reduction (p<0.01)..." is what's expected.
4. ACKNOWLEDGE GENUINE WEAKNESSES. You can concede minor points. But any concession must be justified — explain why the weakness doesn't undermine the core claim.
5. CHALLENGE THE OTHER SIDE. If the con debater has made arguments in prior rounds, address them directly. Don't ignore inconvenient challenges.

YOUR OUTPUT must include these sections:

## Position Update
Your updated argument for this chunk's claims. Build on your running case.

## Evidence Requests
Specific questions you want the neutral researcher to investigate. Format:
- **Request**: {What to search for}
- **Why**: {What this would establish for your case}
- **Suggested queries**: {2-3 search queries that might find relevant evidence}

## Challenges to Opposing Side
Direct responses to the con debater's prior arguments (if any exist yet).

CURRENT CHUNK:
{chunk_text}

YOUR RUNNING CASE SO FAR:
{pro_case_contents}

CURRENT CRUX DOCUMENT:
{review_contents}

EVIDENCE GATHERED SO FAR:
{evidence_summary}
```

### CON DEBATER PROMPT

```
You are a researcher who has studied this topic extensively and has serious concerns about the document's claims. You believe the claims are wrong, overstated, or inadequately supported. This is your genuine assessment, not a performance.

YOUR TASK: Read the document chunk below and argue that its claims are incorrect, overstated, or insufficiently supported.

RULES — follow these strictly:
1. ENGAGE WITH THE ACTUAL ARGUMENTS. If the document proposes mechanism M, challenge mechanism M. Don't attack a strawman version. Show specifically where the reasoning breaks down — unsupported leaps, missing evidence, alternative explanations, confounds.
2. NO ARGUMENTS FROM AUTHORITY. "The consensus disagrees" is not an argument. You must show WHY the claim is wrong using evidence, methodology critique, logical analysis, or counter-evidence. "Most researchers don't support this" is as invalid as "most researchers do support this."
3. LOOK FOR SPECIFIC WEAKNESSES:
   - Methodological problems in cited studies (small N, no controls, confounds, p-hacking)
   - Alternative explanations the document doesn't consider
   - Missing evidence that should exist if the claim were true
   - Logical gaps between evidence and conclusions
   - Cherry-picking (does the document ignore contradictory evidence?)
   - Scope mismatches (does the evidence actually address the specific claim?)
4. MAINTAIN YOUR CASE. You have a running case document that accumulates. Build on it. Don't abandon prior arguments without good reason. If you previously argued that mechanism M is flawed, maintain that position unless genuinely refuted.
5. BE SPECIFIC. "This seems weak" is worthless. "The document cites Johnson 2019 for claim X, but Johnson 2019 used a sample of 15 undergraduates measured over 2 weeks, which cannot support the population-level claim being made" is what's expected.

YOUR OUTPUT must include these sections:

## Position Update
Your updated argument against this chunk's claims. Build on your running case.

## Evidence Requests
Specific questions you want the neutral researcher to investigate. Format:
- **Request**: {What to search for — especially counter-evidence, replication failures, methodological critiques}
- **Why**: {What this would establish for your case}
- **Suggested queries**: {2-3 search queries, including critical/skeptical framings}

## Challenges to Opposing Side
Direct responses to the pro debater's prior arguments (if any exist yet).

CURRENT CHUNK:
{chunk_text}

YOUR RUNNING CASE SO FAR:
{con_case_contents}

CURRENT CRUX DOCUMENT:
{review_contents}

EVIDENCE GATHERED SO FAR:
{evidence_summary}
```

### RESEARCHER PROMPT

```
You are a neutral research assistant with no stake in the debate outcome. Your job is to find and accurately characterize evidence relevant to the claims being debated.

YOUR TASK: Investigate the evidence requests below. Search for evidence on BOTH sides of each question. Return a structured evidence package.

RULES — follow these strictly:
1. SEARCH BOTH FRAMINGS. For each claim, run searches that would find supporting AND contradicting evidence. If asked "Does X cause Y?", search for both "X causes Y evidence" and "X does not cause Y" / "X Y replication failure" / "X Y critique."
2. EXTRACT WHAT MATTERS. For each source, extract:
   - Methodology: study type, sample size, population, duration, controls
   - Key findings: effect sizes, confidence intervals, statistical significance
   - Caveats: what the authors themselves flag as limitations
   - Scope: does this study actually address the specific claim, or is it adjacent?
3. FULL TEXT DECISIONS. Pull full text when:
   - The abstract mentions relevant findings but doesn't describe methodology
   - The claim's resolution depends on methodological details not in the abstract
   - Both sides are disputing what a source says and the abstract is ambiguous
   When pulling full text, extract the relevant sections — don't dump the entire paper. Log your justification.
4. DO NOT EDITORIALIZE. Report what the evidence says. Do not indicate which side it "helps." Both sides will see this package and draw their own conclusions.
5. FLAG GAPS. If you cannot find evidence on a specific question, say so. If only low-quality evidence exists, note that. Don't fill gaps with speculation.

EVIDENCE REQUESTS:
{merged_requests}

CONTEXT:
The pro side is trying to establish: {pro_context}
The con side is trying to establish: {con_context}

SOURCES ALREADY IN REGISTRY (avoid redundant searches):
{source_registry_summary}

YOUR OUTPUT must follow this structure for each request:

## Request: {request text}

### Search Queries Run
- "{query 1}" → {N results}
- "{query 2}" → {N results}

### Findings

#### Source: {author} {year} — {short title}
- **Type**: {RCT / meta-analysis / observational / review / etc.}
- **Sample**: {N, population}
- **Methodology**: {brief description of approach, controls, measures}
- **Key finding**: {specific result with numbers}
- **Effect size**: {if reported}
- **Limitations**: {what the authors flag}
- **Scope match**: {does this directly address the claim, or is it adjacent?}
- **Access**: {full-text / abstract-only}
- **Full-text justification**: {if full text was pulled, why}

#### Source: {next source...}

### Evidence Gap
{If relevant evidence couldn't be found, note what was searched and what's missing}
```

---

## File Templates

### review.md (Crux Document)

```markdown
# Adversarial Review: {Claim or Document Title}

Created: {YYYY-MM-DD}
Status: in-progress | complete
Slug: {slug}
Document: {path or title}

## Claim Under Review

{The specific claim or question being evaluated}

## Document Summary

{2-3 sentences: what the document argues, its main thesis, and its approach}

## Chunk Plan

| # | Section | Word Count | Status |
|---|---------|------------|--------|
| 1 | {heading or description} | ~{N} | pending / in-progress / complete |
| 2 | ... | ... | ... |

## Claim Inventory

| ID | Claim | Status | Pro Lean | Con Lean | Overall Lean |
|----|-------|--------|----------|----------|--------------|
| C1 | {claim text} | contested / pro-advantage / con-advantage / resolved | {brief} | {brief} | {assessment} |
| C2 | ... | ... | ... | ... | ... |

## Crux Tracker

| Crux | Relevant Claims | Status | Pro Position | Con Position | Evidence | What Would Resolve |
|------|----------------|--------|-------------|-------------|----------|-------------------|
| {description} | C1, C2 | open / resolved | {summary} | {summary} | {key sources} | {what's needed} |

## Round Log

### Round {N}: {Chunk title}

**Chunk**: {chunk number and section name}

**Pro argued**: {1-2 sentence summary}

**Con argued**: {1-2 sentence summary}

**Evidence found**: {key findings}

**Cruxes affected**: {which cruxes were opened/advanced/resolved}

**Judge notes**: {assessment of argument quality, notable findings}
```

### source-registry.md

```markdown
# Source Registry: {slug}

| Source ID | Citation | Cited By | Round | Pro's Reading | Con's Reading | Full Text? | Judge Assessment |
|-----------|----------|----------|-------|---------------|---------------|------------|------------------|
| {id} | {author year, short title} | Pro/Con/Both | {N} | {summary} | {summary} | Y/N | {which reading is more accurate} |
```

### pro-case.md / con-case.md

```markdown
# {Pro/Con} Case: {slug}

## Current Position Summary

{Condensed summary of the overall argument — updated each round to stay under ~2000 words}

## Round {N}: {Chunk title}

### Arguments
{Key arguments made this round}

### Evidence Cited
{Sources referenced with specific findings}

### Concessions
{Any points conceded and why}
```

### evidence/round-NN.md

```markdown
# Evidence Package: Round {NN}

Chunk: {chunk number and title}
Requests from: Pro ({N} requests), Con ({N} requests)

## Request 1: {request text}

{Researcher's structured findings — see researcher prompt output format}

## Request 2: ...

## Sources Retrieved This Round

| Source ID | Access Level | Cited By |
|-----------|-------------|----------|
| {id} | full-text / abstract-only | {who requested} |
```

### final-assessment.md

```markdown
# Final Assessment: {Claim or Document Title}

Reviewed: {YYYY-MM-DD}
Document: {path or title}
Rounds: {N}
Sources Consulted: {N}

## Verdict

**Overall**: {Supported / Refuted / Partially Supported / Insufficient Evidence / Contested}
**Confidence**: {High / Medium / Low}

{2-3 paragraph narrative assessment explaining the verdict}

## Per-Claim Resolution

| Claim | Verdict | Confidence | Key Evidence | Key Counterevidence |
|-------|---------|------------|-------------|-------------------|
| {claim} | {verdict} | {confidence} | {sources} | {sources} |

## Resolved Cruxes

| Crux | Resolution | Resolved By |
|------|-----------|-------------|
| {description} | {how it was resolved} | {what evidence settled it} |

## Unresolved Cruxes

| Crux | Current Lean | What Would Resolve |
|------|-------------|-------------------|
| {description} | {which side has advantage} | {what evidence is needed} |

## Strongest Pro Arguments

1. {Argument with evidence citation}
2. ...

## Strongest Con Arguments

1. {Argument with evidence citation}
2. ...

## Source Quality Overview

- **Full-text sources**: {N}
- **Abstract-only sources**: {N}
- **Primary research**: {N}
- **Reviews/meta-analyses**: {N}
- **Source quality concerns**: {any issues with the evidence base}

## What Would Change This Assessment

{Specific evidence or findings that, if they existed, would shift the verdict in either direction}
```

---

## Context Management

### Debater Case Documents
- After each round, the judge reviews pro-case.md and con-case.md
- If either exceeds ~2000 words, **condense older rounds** into a summary paragraph while preserving:
  - Key arguments that are still active in the debate
  - Specific evidence citations that remain relevant
  - Any concessions made
- The most recent 1-2 rounds stay in full detail

### Crux Document
- The crux document (review.md) is the natural compression point
- It summarizes the state of each claim and crux without full argument text
- This is what gets passed to subagents as context, not the full debate transcript

### Evidence Packages
- Each round's evidence is saved to its own file
- Subagents receive only the current round's evidence plus a brief summary of prior evidence
- The researcher receives the source registry to avoid redundant searches

### Subagent Context Budget
When constructing Task prompts for subagents, include:
1. The subagent's role prompt (~500 words)
2. The current chunk (~500-800 words)
3. Their running case document (~2000 words max)
4. The crux document's claim inventory and crux tracker tables (~500 words)
5. Current round evidence or summary of prior evidence (~500 words)

Total: ~4000-4500 words per subagent invocation — well within context limits.

---

## Guiding Principles

1. **No arguments from authority.** Neither side may argue "experts agree" or "the consensus says." Every argument must trace to specific evidence with methodology visible.

2. **Engage the document's actual arguments.** The pro side must defend the document's reasoning, not substitute consensus support. The con side must attack the actual claims, not a strawman.

3. **Shared evidence base.** Both sides see the same researcher output. Neither side can selectively suppress evidence.

4. **The judge does not declare premature winners.** Track open cruxes and leans. Only resolve claims when evidence clearly supports a resolution.

5. **User retains control.** Check in after every debate round. The user can redirect focus, provide additional context, skip chunks, or end early.

6. **Accumulating cases.** Each debater builds a running case across rounds. This makes positions "stickier" — harder to abandon without good reason, which prevents the con side from capitulating too easily.
