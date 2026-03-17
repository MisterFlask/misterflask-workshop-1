---
name: deep-research
description: Conduct systematic literature research with source tracking, digests, and synthesis. Use for academic literature review, finding primary sources, and building compaction-resistant research artifacts.
allowed-tools: Read, Write, Edit, Glob, Grep, WebSearch, WebFetch, Bash(mkdir:*)
user-invocable: true
---

# Deep Research Skill

Systematic literature research with metadata tracking, digest generation, and cross-source synthesis. Designed to produce compaction-resistant artifacts that remain useful after context loss.

## Directory Structure

All research artifacts go in `research/` at repository root:

```
research/
├── questions/           # Research question files (the "hub")
│   └── {topic-slug}.md
├── sources/             # Source metadata + context
│   └── {topic-slug}/
│       └── {source-id}.md
├── digests/             # Compaction-resistant summaries
│   └── {topic-slug}/
│       ├── index.md     # Master synthesis across sources
│       └── {source-id}.md
└── transcripts/         # Full PDF transcriptions (when provided)
    └── {source-id}.md
```

## Workflow

### Phase 1: Initialize Research Question

When user provides a research question:

1. **Create slug**: Convert question to kebab-case slug (e.g., "mct-vs-cbt-social-anxiety")

2. **Create directories**:
   ```bash
   mkdir -p research/questions research/sources/{slug} research/digests/{slug} research/transcripts
   ```

3. **Create question file** at `research/questions/{slug}.md` using template below

4. **Identify relevant blog context** by scanning `posts/` for related content

### Phase 2: Search & Discover

1. **Run searches** using WebSearch:
   - Google Scholar queries
   - PubMed queries
   - Author name searches for key researchers
   - Citation chaining from known good papers

2. **Document searches** in the question file's Search Log section

3. **For each relevant source found**:
   - Generate source ID: `{first-author-lastname}-{year}` (e.g., `wells-2009`)
   - Add to question file's source table
   - Create source file and digest file

### Phase 3: Fetch & Digest

For each source:

1. **Attempt access** via WebFetch:
   - Try publisher page
   - Try PubMed abstract
   - Try Google Scholar cached version
   - Note what level of access was achieved

2. **Create source file** at `research/sources/{slug}/{source-id}.md`

3. **Create digest file** at `research/digests/{slug}/{source-id}.md`

4. **Flag if upgrade needed**: If abstract-only but highly relevant, add prominent NEED FULL TEXT section

### Phase 4: Synthesize

After gathering sources:

1. **Create/update index.md** in the digest folder
2. **Note conflicts** between sources
3. **Identify gaps** in the literature
4. **Connect to research question** explicitly

### Phase 5: Full Text Integration

When user provides full text (PDF or pasted):

1. **Transcribe to markdown** in `research/transcripts/{source-id}.md`:
   - Preserve section structure
   - Describe figures/tables with key data points
   - Include page breaks for reference

2. **Update digest** with new information from full text

3. **Update source file** access status to `full-text`

4. **Remove NEED FULL TEXT flag** from digest

5. **Update index.md** synthesis if findings changed

---

## File Templates

### Research Question File

```markdown
# Research Question: {Title}

Created: {YYYY-MM-DD}
Status: active | paused | complete
Slug: {topic-slug}

## Core Question

{The specific question being investigated. Be precise.}

## Context & Motivation

{Why this matters. Links to relevant blog posts. How this connects to broader thesis.}

## Sources

| ID | Title | Year | Access | Priority | Notes |
|----|-------|------|--------|----------|-------|
| {id} | {short title} | {year} | full-text/abstract-only/need-full-text/inaccessible | high/medium/low | {brief note} |

## Search Log

| Date | Query | Database | Results | Notes |
|------|-------|----------|---------|-------|
| {date} | "{query}" | Scholar/PubMed | {count} | {what was useful} |

## Key Findings (Running)

{Updated as sources are digested. Each finding must be followed by the full verbatim paragraph from the source in which the claim appears, as a blockquote with source attribution. Format:}

{Finding statement.}

> {Full verbatim paragraph containing the claim.}
>
> — {source-id}, {section name or page}}

## Open Questions

{What remains unclear. What would resolve current uncertainties.}

## Conflicts & Tensions

{Where sources disagree. Unresolved debates.}
```

### Source File

```markdown
# {Full Title}

Source ID: {source-id}
Question: [{question-title}](../questions/{slug}.md)

## Bibliographic Info

- **Authors**: {list}
- **Year**: {year}
- **Journal**: {journal name}
- **DOI**: {doi if available}
- **URL**: {original URL}
- **Sci-Hub**: {leave blank for user to fill}

## Access Status

**Current**: full-text | abstract-only | inaccessible

**Access Notes**: {How access was obtained. What's behind paywall. etc.}

## Abstract

{Copy of abstract if available}

## Citation Notes

{Key papers this cites that might be worth following up}
```

### Digest File

```markdown
# Digest: {Short Title}

Source: [{source-id}](../../sources/{slug}/{source-id}.md)
Access: full-text | abstract-only
Last Updated: {YYYY-MM-DD}

## Summary

{2-4 sentences. What is this paper about? What did they do?}

## Key Findings

- {Main results with effect sizes where available}
- {Sample sizes in parentheses}
- {Statistical significance noted}

## Operationalization of Key Concepts

{CRITICAL: For each important concept or measurement in the study, explain HOW it was operationalized. This is essential for evaluating whether findings actually support conclusions.}

| Concept | How Measured/Operationalized | Notes |
|---------|------------------------------|-------|
| {e.g., "exposure use"} | {e.g., "Self-report: 'Do you use exposure therapy?' yes/no"} | {e.g., "Doesn't distinguish in-session vs homework"} |
| {e.g., "therapist anxiety"} | {e.g., "Anxiety Sensitivity Index (ASI-3), 18 items"} | {e.g., "Validated scale, measures fear of anxiety symptoms"} |
| {e.g., "safety behavior"} | {e.g., "Asked if they prescribed controlled breathing during IE"} | {e.g., "Binary question, 70% said 'to make it less aversive'"} |

**Operationalization concerns**: {Note any cases where the operationalization is weak, ambiguous, or doesn't match how the concept is discussed in conclusions. Flag if abstract-only and operationalization details are unknown.}

## Methodology

- **Study Type**: {RCT, meta-analysis, observational cohort, case study, review, etc.}
- **Sample**: {n=X, population characteristics, recruitment}
- **Measures**: {What instruments/scales used}
- **Duration**: {Follow-up period if applicable}

## Methodological Critique

### Strengths
- {What they did well}

### Limitations
- {Confounds, generalizability issues, measurement problems}
- {What would make this stronger}

## Relevance to Research Question

{How does this connect to the specific question being investigated?}

## Connections to Other Sources

{Does this support, contradict, or extend other sources in this collection?}

---

## NEED FULL TEXT

> **Priority**: HIGH
>
> **What's Missing**: {Specific sections that would be valuable - methods details, full results tables, discussion of mechanisms, etc.}
>
> **Operationalization Gap**: {What key concepts can't be evaluated because we don't know how they were measured? e.g., "Can't assess how 'exposure use' was operationalized - could be binary yes/no or frequency scale, in-session vs homework, etc."}
>
> **Why It Matters**: {Why upgrading this source is important for the research question}

{DELETE THIS SECTION when full text is obtained}
```

### Index/Synthesis File

```markdown
# Synthesis: {Research Question Title}

Question: [{slug}](../../questions/{slug}.md)
Sources Reviewed: {count}
Last Updated: {YYYY-MM-DD}

## Executive Summary

{3-5 sentences. What does the evidence say overall? What's the confidence level?}

## Key Themes

### {Theme 1}
{What the evidence shows. Which sources support this. Strength of evidence.}

### {Theme 2}
{...}

## Points of Consensus

- {What most/all sources agree on}

## Active Debates

- {Where sources disagree and why}
- {What would resolve the disagreement}

## Gaps in Literature

- {Questions that aren't well-addressed}
- {Populations/contexts understudied}

## Implications for {Blog/Thesis Context}

{How this connects to the broader project. What it suggests for the thesis being developed.}

## Sources by Relevance

### High Relevance
- [{source-id}](./{source-id}.md): {one-line summary}

### Medium Relevance
- [{source-id}](./{source-id}.md): {one-line summary}

### Background/Context Only
- [{source-id}](./{source-id}.md): {one-line summary}
```

### Transcript File

```markdown
# Transcript: {Full Title}

Source: [{source-id}](../sources/{slug}/{source-id}.md)
Transcribed: {YYYY-MM-DD}
Pages: {count}

---

## {Section Title}

{Verbatim text from PDF}

### {Subsection}

{Verbatim text}

---

**[Figure 1]**: {Description of figure. Key data points. What it shows.}

**[Table 1]**: {Description or markdown recreation of table with key values.}

---

{Continue for full document}
```

---

## Source ID Convention

Format: `{first-author-lastname}-{year}`

Examples:
- `wells-2009` (Adrian Wells, 2009)
- `clark-1995` (David Clark, 1995)
- `hofmann-2012` (Stefan Hofmann, 2012)

If collision (same author, same year): append letter (`wells-2009a`, `wells-2009b`)

---

## Access Status Definitions

| Status | Meaning |
|--------|---------|
| `full-text` | Complete paper read and digested |
| `abstract-only` | Only abstract available; digest based on abstract |
| `need-full-text` | Abstract-only BUT high relevance; user should find full text |
| `inaccessible` | Couldn't even get abstract; noted for completeness |

---

## Priority Flags

In the question file's source table:

- **HIGH**: Directly addresses core question; upgrade to full-text if possible
- **medium**: Relevant but not central
- **low**: Background/context only

The NEED FULL TEXT section in digests should be prominent and explain *why* the full text matters.

---

## PDF Transcription Guidelines

When user provides a PDF:

1. **Read the PDF** using the Read tool (it handles PDFs)

2. **Transcribe verbatim** to markdown:
   - Preserve all section headers
   - Keep paragraph structure
   - Note page breaks with `---`

3. **Handle visuals**:
   - Figures: Describe what's shown, note key data points, trends
   - Tables: Recreate in markdown if simple; describe if complex
   - Charts: Describe axes, key values, patterns

4. **After transcription**:
   - Update the digest with new information
   - Update source file access status
   - Remove NEED FULL TEXT flag
   - Update index.md if synthesis changes

---

## Search Strategy

### Databases (preference order)
1. Google Scholar (broad coverage)
2. PubMed (medical/clinical)
3. PsycINFO via web search (psychology-specific)

### Query Patterns
- Core concept search: `"metacognitive therapy" "social anxiety"`
- Author search: `author:"Adrian Wells" MCT`
- Citation search: find papers citing a known good source
- Recency filter: add year ranges for recent work

### Citation Chaining
When a good source cites other promising papers:
1. Note them in the source file's Citation Notes
2. Flag high-priority ones for follow-up
3. Add to question file with status `inaccessible` until fetched

---

## Citation Standard for the Question File

**Rule**: Whenever a claim from a source is recorded in the question file — in Key Findings, Open Questions, or Conflicts & Tensions — you MUST include the full verbatim paragraph from the source that contains the claim, as a blockquote immediately following the claim, with source attribution.

**Format**:
```
{Claim or finding statement.}

> {Full verbatim paragraph from source.}
>
> — {source-id}, {section or page reference}
```

**Why**: The question file captures evolving analytical conclusions. Anchoring each claim to its source paragraph prevents distortion across sessions and makes it possible to verify whether the claim accurately represents what the source says.

**Scope**: This applies to all claim-bearing content added to the question file, including:
- Items under Key Findings (Running)
- Items under Open Questions
- Items under Conflicts & Tensions

This does NOT apply to the Sources table or Search Log (those are metadata, not claims).

If a claim synthesizes multiple sources, quote the most specific/direct paragraph and note the others with `(also: {source-id})`.

If the source is abstract-only and the full paragraph is not available, write `[Full paragraph unavailable — abstract-only]` instead of a blockquote.

---

## Follow-up Q&A Tracking

When the user asks a follow-up question, distinguish between two types:

### Type 1: Factual Questions About Specific Papers

Questions like "What was the sample size in Whiteside 2016?" or "Did Deacon 2013 control for therapist experience?"

1. **Check the digest index.md** to see if already answered
2. **If NOT answered**, add to a "Follow-up Q&A" section at the end of **index.md**:

```markdown
## Follow-up Q&A

### Q: {User's question}
**Asked**: {YYYY-MM-DD}

{Answer based on the specific source(s)}

**Source**: {source-id}

---
```

3. If it reveals missing information, note in "Gaps in Literature"

### Type 2: Broad Analytical Questions

Questions like "What does this imply for self-directed treatment?" or "How does this connect to the MCT literature?"

These are **general interpretive questions** that go beyond specific study facts. Add these to the **research question file** (`research/questions/{slug}.md`) in the "Open Questions" or "Key Findings" sections, NOT to the digest.

This keeps the digest focused on what the literature says, while the question file captures the user's evolving analytical thinking.

---

## Invocation

User can invoke with:
- `/deep-research` - Start new research question
- `/deep-research add {source-id}` - Add source to current question
- `/deep-research digest {source-id}` - Create/update digest for source
- `/deep-research synthesize` - Update index.md synthesis
- `/deep-research status` - Show current question status

Or just describe what they want naturally and this skill will be applied.
