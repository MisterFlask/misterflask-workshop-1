---
name: job-matcher
description: Search job boards and match listings against a resume using LLM-based fuzzy matching. Use when asked to find jobs, search for positions, or match a resume to job listings.
allowed-tools: Read, Write, Bash(python:*), Bash(pip:*), Glob, Grep
---

# Job Matcher

Find job listings from multiple job boards and rank them by how well they match a given resume, using LLM-based fuzzy matching rather than keyword search.

## Workflow

### Phase 1: Setup & Fetch

#### Step 1: Identify Resume
Ask user for the resume file path, or use the default sample at `projects/job-matcher/sample_resume.md`.

#### Step 2: Read Resume
Read the resume file completely. Internalize the candidate's:
- Core skills and experience areas
- Languages spoken
- Industry experience
- Education and certifications
- Job preferences (remote, location, etc.) if mentioned

#### Step 3: Install Dependencies
```bash
pip install -r projects/job-matcher/requirements.txt
```

#### Step 4: Fetch Jobs
Run the job fetcher script to pull listings from all configured APIs:

```bash
python projects/job-matcher/fetch_jobs.py --output projects/job-matcher/fetched_jobs.json
```

Default location is Minneapolis, MN. Default search terms are: translator, photographer, bilingual, content creator.

**Sources (always active, no keys needed):**
- Remotive (remote jobs)
- RemoteOK (remote jobs)
- Jobicy (remote creative/marketing/copywriting jobs)

**Sources (active when env vars are set):**
- Jooble (general aggregator, all categories) — set `JOOBLE_API_KEY`
- USAJobs (federal government — great for translator/interpreter roles) — set `USAJOBS_API_KEY` + `USAJOBS_EMAIL`
- Adzuna (general aggregator, salary data) — set `ADZUNA_APP_ID` + `ADZUNA_APP_KEY`, pass `--adzuna` flag

Optional flags:
- `--location LOC` — change location (default: "Minneapolis, MN")
- `--search TERM` — override search terms (can repeat)
- `--category CAT` — filter Remotive by category (can repeat)
- `--adzuna` — include Adzuna results
- `--adzuna-query Q` — custom Adzuna search query
- `--country CC` — Adzuna country code (default: us)

Report to user how many jobs were fetched and from which sources.

### Phase 2: Match & Rank

#### Step 5: Read Fetched Jobs
Read the JSON file at `projects/job-matcher/fetched_jobs.json`.

#### Step 6: Evaluate Each Job
For each job listing, evaluate how well the candidate matches. Consider:

1. **Direct skill matches** — does the job explicitly ask for skills the candidate has?
2. **Transferable skills** — would the candidate's experience translate? (e.g., "bilingual" matches "Spanish translator"; "content creator" matches a photographer)
3. **Cultural/industry fit** — does the candidate's background suggest they'd thrive here?
4. **Underqualified vs. overqualified** — is this a stretch, a fit, or beneath their experience?
5. **Hidden matches** — job titles that sound unrelated but actually need this candidate's exact skill combo

Assign each job:
- **Score (1-10)**: 1 = no match, 5 = possible fit, 8+ = strong match
- **Reasoning**: 1-2 sentences explaining WHY it matches or doesn't
- **Key matches**: Which specific resume elements align with job requirements

**IMPORTANT**: Be generous with scores for jobs where the candidate's unusual skill combination (e.g., bilingual + photography + content creation) is a genuine differentiator, even if the job title doesn't obviously match. The whole point of LLM matching is catching what keyword search misses.

#### Step 7: Process in Batches
To manage context effectively:
- Process jobs in batches of ~30-50
- Keep a running list of jobs scoring 5+
- Discard low-scoring jobs as you go to save context space
- If there are more than 200 jobs total, prioritize variety across sources and categories

### Phase 3: Output

#### Step 8: Generate Results
Write results to a markdown file at `projects/job-matcher/job_matches_YYYY-MM-DD.md` (use today's date).

Format:

```markdown
# Job Matches — [Date]

**Resume**: [filename]
**Jobs scanned**: [total count]
**Matches found**: [count scoring 5+]
**Sources**: [list all sources that returned results]

---

## Top Matches (Score 8-10)

### [Score]/10 — [Job Title]
**Company**: [name]
**Location**: [location]
**Salary**: [if available]
**Source**: [source name]
**Why it matches**: [reasoning]
**Key skills aligned**: [list]
**Apply**: [url]

---

## Good Matches (Score 6-7)

[same format]

---

## Worth a Look (Score 5)

[same format, condensed — just title, company, one-line reason, link]
```

#### Step 9: Summary
Tell the user:
- How many jobs were scanned
- How many matched at each tier
- Top 3 most interesting/unexpected matches
- Any patterns noticed (e.g., "lots of content creator roles want bilingual candidates")
- The output file path

## Key Principles

1. **LLM matching > keyword matching.** The candidate has a unique skill combination. A job titled "Marketing Coordinator" might be a perfect fit if it needs bilingual content creation + product photography. Catch these.

2. **Explain the match.** Scores without reasoning aren't useful. The candidate needs to understand WHY a job matches so they can tailor their application.

3. **Cast wide, rank tight.** Fetch broadly, but be honest in scoring. A 7 should genuinely be worth applying to.

4. **Respect rate limits.** The fetch script handles API politeness. Don't re-run it unnecessarily.
