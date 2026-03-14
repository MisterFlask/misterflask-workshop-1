---
name: paper-retrieval
description: Retrieve full-text academic papers by DOI from open-access sources (PMC, Semantic Scholar, Unpaywall, Sci-Hub). Integrates with deep-research artifacts.
allowed-tools: Read, Write, Edit, Glob, Grep, WebSearch, WebFetch, Bash(mkdir:*), Bash(curl:*), AskUserQuestion
user-invocable: true
---

# Paper Retrieval Skill

Retrieve full-text academic papers by DOI using free/open-access APIs. Designed to solve the "abstract-only" problem in research workflows by systematically attempting multiple sources for full text.

Integrates with the `deep-research` skill's directory structure: updates source files, digests, and creates transcripts when full text is obtained.

---

## Invocation

- `/paper-retrieval <DOI>` - Retrieve a single paper by DOI
- `/paper-retrieval <DOI> [topic-slug]` - Retrieve and link to an existing research question
- `/paper-retrieval batch [topic-slug]` - Scan a research question's sources for `need-full-text` entries and attempt retrieval for all of them
- `/paper-retrieval status [topic-slug]` - Show access status summary for a research question

Or describe naturally: "get the full text for 10.1016/j.brat.2013.04.002" or "try to find full texts for all my social anxiety sources"

---

## Retrieval Pipeline

For each DOI, attempt sources in this order. Stop as soon as full text is obtained.

### Source 1: PubMed Central (PMC)

PMC hosts full text for open-access and NIH-funded papers. Best coverage for biomedical/clinical literature.

**Step 1a — Convert DOI to PMCID:**

```
WebFetch: https://www.ncbi.nlm.nih.gov/pmc/utils/idconv/v1.0/?ids={DOI}&format=json
```

Look for `pmcid` in the response. If no PMCID exists, skip to Source 2.

**Step 1b — Fetch full text via BioC API:**

```
WebFetch: https://www.ncbi.nlm.nih.gov/research/bionlp/RESTful/pmcoa.cgi/BioC_json/{PMCID}/unicode
```

This returns structured JSON with full text divided into passages. If BioC fails, try the PMC HTML page:

```
WebFetch: https://www.ncbi.nlm.nih.gov/pmc/articles/{PMCID}/
```

**Parse the BioC JSON response:**
- The response contains `documents[0].passages[]` with `infons.section_type` and `text` fields
- Section types include: `TITLE`, `ABSTRACT`, `INTRO`, `METHODS`, `RESULTS`, `DISCUSS`, `CONCL`, `REF`, `TABLE`, `FIG`
- Reconstruct the paper by iterating passages in order, using section_type as headers
- For tables: look for `infons.type: "table"` passages and format as markdown tables
- For figures: create descriptive placeholders from `infons.type: "fig_caption"` passages

### Source 2: Semantic Scholar API

Free API with good coverage. Returns metadata and open-access PDF links where available.

```
WebFetch: https://api.semanticscholar.org/graph/v1/paper/DOI:{DOI}?fields=title,abstract,authors,year,externalIds,openAccessPdf,tldr,citationCount,referenceCount
```

**What to extract:**
- `openAccessPdf.url` — Direct link to a legal OA PDF. If present, fetch and transcribe it.
- `tldr.text` — AI-generated summary (useful supplement, not a replacement for full text)
- `externalIds.PubMedCentral` — Alternative way to get PMCID if Source 1 failed
- `externalIds.ArXiv` — ArXiv ID for preprint access
- `authors`, `year`, `citationCount` — Useful metadata for source files

**If `openAccessPdf.url` is present:**

PDF URLs must be downloaded locally before reading — WebFetch cannot parse binary PDF content. Use the **PDF download procedure** below.

**If ArXiv ID is present:**

```
WebFetch: https://arxiv.org/abs/{arxivId}
```

ArXiv papers are always freely available. The HTML version is often easier to parse than the PDF.

### Source 3: Unpaywall API

Finds legal open-access versions of papers (green OA, gold OA, bronze OA).

```
WebFetch: https://api.unpaywall.org/v2/{DOI}?email=research-tool@example.com
```

**What to extract:**
- `best_oa_location.url_for_pdf` — Direct PDF URL
- `best_oa_location.url_for_landing_page` — Landing page with access
- `best_oa_location.host_type` — "publisher" (gold OA) or "repository" (green OA)
- `best_oa_location.version` — "publishedVersion", "acceptedVersion", or "submittedVersion"
- `oa_locations[]` — All available OA locations (try multiple if first fails)
- `is_oa` — Boolean: whether any OA version exists

**If a URL is found**, download and transcribe using the **PDF download procedure** below.

Note the version in the transcript (accepted manuscripts may differ from published version).

### Source 4: Europe PMC

Broader coverage than US PMC. Sometimes has full text when PMC doesn't.

```
WebFetch: https://www.ebi.ac.uk/europepmc/webservices/rest/search?query=DOI:{DOI}&format=json&resultType=core
```

If `fullTextUrlList` contains entries, follow the `availabilityCode: "OA"` links.

### Source 5: CrossRef (Metadata Enrichment)

Even if full text isn't found, CrossRef provides rich metadata that improves source files.

```
WebFetch: https://api.crossref.org/works/{DOI}
```

Extract: full title, all authors with affiliations, journal, volume/issue/pages, references list, funding info, license info.

### Source 6: Sci-Hub (Automated Fallback)

If all open-access sources fail, attempt Sci-Hub programmatically. The domain changes frequently — try these in order:

**Step 6a — Attempt fetch:**

```
WebFetch: https://sci-hub.se/{DOI}
```

If that fails (timeout, 403, domain dead), try alternate domains:
- `https://sci-hub.st/{DOI}`
- `https://sci-hub.ru/{DOI}`

**Step 6b — Parse the response:**

Sci-Hub pages typically embed the PDF in an `<iframe>` or `<embed>` tag. Look for:
- A URL ending in `.pdf` in the page content
- An `<iframe src="...">` pointing to the PDF
- A direct download link

If a PDF URL is found, use the **PDF download procedure** below.

**Step 6c — If Sci-Hub is blocked or unresponsive:**

Present the URL to the user:

> Sci-Hub appears to be blocked or the domain has changed. You can try manually at: `https://sci-hub.se/{DOI}`
>
> If you obtain the PDF, provide it and I'll transcribe it into the research collection.

**Step 6d — If the user provides a PDF or pasted text after Sci-Hub fails:**

Proceed with transcription as normal (Phase 5 of deep-research workflow).

---

## PDF Download Procedure

Use this procedure whenever a direct PDF URL is obtained (from Semantic Scholar `openAccessPdf.url`, Unpaywall `url_for_pdf`, Sci-Hub embed, or any other source). **Do not use WebFetch on PDF URLs** — it cannot parse binary content.

**Step 1 — Download to a temp file:**

```bash
curl -L -o /tmp/{source-id}.pdf "{pdf-url}"
```

The `-L` flag follows redirects. If `curl` returns a non-zero exit code or the file is under 10KB (likely an error page, not a real PDF), mark as failed and try the next source.

**Step 2 — Check it's actually a PDF:**

```bash
file /tmp/{source-id}.pdf
```

Output should contain `PDF document`. If it says `HTML document` or similar, the URL redirected to a paywall/login page — mark as failed.

**Step 3 — Read the PDF using the Read tool:**

The `Read` tool handles PDFs natively. For papers under ~10 pages, read without page range:

```
Read: /tmp/{source-id}.pdf
```

For longer papers (>10 pages), read in chunks of up to 20 pages:

```
Read: /tmp/{source-id}.pdf  pages: "1-20"
Read: /tmp/{source-id}.pdf  pages: "21-40"
...continue until end of document...
```

To determine total page count before chunking:

```bash
python3 -c "import subprocess; r=subprocess.run(['pdfinfo','/tmp/{source-id}.pdf'],capture_output=True,text=True); print(r.stdout)" 2>/dev/null || echo "pdfinfo unavailable"
```

If `pdfinfo` is unavailable, start reading from page 1 and continue in 20-page chunks until the Read tool returns an empty result or an error.

**Step 4 — Transcribe the content:**

Assemble the text from all chunks into the transcript file. Preserve section structure, describe figures, recreate simple tables in markdown.

**Step 5 — Clean up:**

```bash
rm -f /tmp/{source-id}.pdf
```

---

## Retrieval Result Handling

### Full Text Obtained

When full text is successfully retrieved from any source:

1. **Create transcript** at `research/transcripts/{source-id}.md` using the transcript template from deep-research:
   - Preserve section structure (Introduction, Methods, Results, Discussion, etc.)
   - Describe figures with key data points
   - Recreate simple tables in markdown; describe complex ones
   - Note the access source (e.g., "Retrieved from PMC via BioC API")

2. **Update source file** (if it exists at `research/sources/{slug}/{source-id}.md`):
   - Set Access Status to `full-text`
   - Add access notes about where the full text was found
   - Add any enriched metadata (CrossRef, Semantic Scholar)

3. **Update digest file** (if it exists at `research/digests/{slug}/{source-id}.md`):
   - Remove the `## NEED FULL TEXT` section
   - Update Access field to `full-text`
   - Enrich Key Findings, Methodology, and Operationalization sections with full-text details
   - Update Methodological Critique with information only available in full text

4. **Update question file** source table — change access status column

5. **Update index.md** if the new information changes the synthesis

### Abstract Only (No Full Text Found)

When all sources fail to provide full text:

1. **Collect and merge metadata** from all APIs that responded
2. **Create/update source file** with enriched metadata (CrossRef gives the richest)
3. **Ensure digest has** a prominent `## NEED FULL TEXT` section
4. **Report to user** which sources were tried and what was found
5. **Provide Sci-Hub URL** as user-directed fallback

### Partial Text

Some sources provide partial content (e.g., first page only, accepted manuscript without figures):

1. **Note the limitation** in the transcript and source file
2. **Set access status** to `partial-text` with explanation
3. **Still update digest** with whatever new information was obtained
4. **Keep NEED FULL TEXT** flag but note what was already obtained

---

## Batch Mode

When invoked with `/paper-retrieval batch [topic-slug]`:

1. **Scan the question file** at `research/questions/{slug}.md`
2. **Identify sources with** `need-full-text` or `abstract-only` + `high` priority
3. **Extract DOIs** from corresponding source files in `research/sources/{slug}/`
4. **Process each DOI** through the retrieval pipeline
5. **Report results** as a summary table:

```
## Batch Retrieval Results for {slug}

| Source ID | DOI | Result | Access Source | Notes |
|-----------|-----|--------|---------------|-------|
| wells-2009 | 10.xxx | full-text | PMC/BioC | Transcribed |
| clark-1995 | 10.xxx | abstract-only | No OA found | Sci-Hub URL provided |
| ... | ... | ... | ... | ... |

Upgraded: X/Y sources
Still need full text: Z sources
```

---

## Status Mode

When invoked with `/paper-retrieval status [topic-slug]`:

1. Read the question file
2. Count sources by access status
3. Show which high-priority sources still need full text
4. Suggest next steps

---

## Standalone Mode (No Existing Research Question)

When a DOI is provided without a topic-slug and no matching research question exists:

1. **Still run the full retrieval pipeline**
2. **Create the transcript** at `research/transcripts/{source-id}.md`
3. **Create a minimal source file** at `research/sources/standalone/{source-id}.md`
4. **Print a summary** of what was found: title, authors, year, abstract, and whether full text was obtained
5. **Note**: The user can later link this to a research question with `/deep-research add {source-id}`

---

## API Notes & Rate Limits

| API | Rate Limit | Auth Required | Notes |
|-----|------------|---------------|-------|
| PMC ID Converter | Generous | No | Batch up to 200 IDs |
| PMC BioC | Generous | No | Best structured full text |
| Semantic Scholar | 100/5min (unauth) | Optional API key | `x-api-key` header for higher limits |
| Unpaywall | 100k/day | Email as param | Use any valid email |
| Europe PMC | Generous | No | Good for European journals |
| CrossRef | 50/sec (polite pool) | No | Add `mailto` param for polite pool |
| Sci-Hub | N/A | No | User-directed only; domain changes |

**Important**: WebFetch may not render all API responses correctly. If a JSON API response is truncated or mangled, fall back to `Bash(curl:*)`:

```bash
curl -s "https://api.semanticscholar.org/graph/v1/paper/DOI:10.1016/j.brat.2013.04.002?fields=title,abstract,openAccessPdf"
```

---

## Troubleshooting

### DOI Format
- Strip any URL prefix: `https://doi.org/10.1016/...` → `10.1016/...`
- DOIs are case-insensitive but preserve original case for consistency
- URL-encode special characters in DOIs when used in URLs (e.g., `<` → `%3C`)

### Common Failures
- **PMC returns no PMCID**: Paper isn't in PMC. Move to next source.
- **Semantic Scholar 404**: DOI might not be indexed. Try searching by title instead.
- **Unpaywall returns `is_oa: false`**: No legal OA version found. Provide Sci-Hub URL.
- **WebFetch returns garbled content**: Try `Bash(curl:*)` instead for API calls.
- **Rate limited (429)**: Wait 30 seconds and retry. For Semantic Scholar, consider requesting an API key.

### When Nothing Works
If all APIs fail and no OA version exists:
1. Check if the paper has a preprint version (search title on Google Scholar with `filetype:pdf`)
2. Check the author's personal/lab website (often hosts accepted manuscripts)
3. Check institutional repositories (search title + "repository" or "accepted manuscript")
4. Provide Sci-Hub URL as final fallback
5. Note the paper as `inaccessible` with explanation
