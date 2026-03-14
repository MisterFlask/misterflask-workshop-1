#!/usr/bin/env python3
"""
Fetch jobs from multiple APIs and output normalized JSON.

This script is the data-fetching layer for the job-matcher skill.
It pulls jobs from multiple sources, normalizes them, and writes
to a JSON file for Claude Code to process.

Sources (no API key needed):
  - Remotive (remote jobs)
  - RemoteOK (remote jobs)
  - Jobicy (remote jobs, creative/marketing categories)

Sources (free API key required):
  - Jooble (general aggregator) — JOOBLE_API_KEY
  - Adzuna (general aggregator) — ADZUNA_APP_ID + ADZUNA_APP_KEY
  - USAJobs (federal government) — USAJOBS_API_KEY + USAJOBS_EMAIL

Usage:
    python fetch_jobs.py [options]
"""

import argparse
import json
import sys
import time
from pathlib import Path

# Add project root to path so we can import apis/
sys.path.insert(0, str(Path(__file__).parent))

from apis import remotive, remoteok, adzuna, jooble, usajobs, jobicy


def _log(msg: str):
    print(msg, file=sys.stderr)


def fetch_all(args: argparse.Namespace) -> list[dict]:
    """Fetch from all configured sources and combine."""
    all_jobs = []
    search_terms = args.search or []
    location = args.location

    # --- Free sources (no API key) ---

    # Remotive
    _log("Fetching from Remotive...")
    try:
        categories = args.category or [None]
        for cat in categories:
            terms = search_terms or [None]
            for term in terms:
                jobs = remotive.fetch_jobs(category=cat, search=term)
                all_jobs.extend(jobs)
                _log(f"  Remotive: {len(jobs)} jobs (category={cat}, search={term})")
    except Exception as e:
        _log(f"  Remotive error: {e}")

    # RemoteOK
    time.sleep(1)
    _log("Fetching from RemoteOK...")
    try:
        jobs = remoteok.fetch_jobs()
        all_jobs.extend(jobs)
        _log(f"  RemoteOK: {len(jobs)} jobs")
    except Exception as e:
        _log(f"  RemoteOK error: {e}")

    # Jobicy
    time.sleep(1)
    _log("Fetching from Jobicy...")
    try:
        # Fetch across several creative/non-tech industries
        industries = ["marketing", "copywriting", "design-multimedia", "business"]
        jobicy_total = 0
        for industry in industries:
            try:
                jobs = jobicy.fetch_jobs(count=50, industry=industry)
                all_jobs.extend(jobs)
                jobicy_total += len(jobs)
            except Exception as e:
                _log(f"  Jobicy ({industry}): {e}")
        _log(f"  Jobicy: {jobicy_total} jobs across {len(industries)} industries")
    except Exception as e:
        _log(f"  Jobicy error: {e}")

    # --- API key sources (skip silently if keys not set) ---

    # Jooble
    _log("Fetching from Jooble...")
    try:
        # Run multiple searches to cast a wide net
        jooble_terms = search_terms or ["translator", "photographer", "bilingual", "content creator"]
        jooble_total = 0
        for term in jooble_terms:
            jobs = jooble.fetch_jobs(keywords=term, location=location, max_pages=2)
            all_jobs.extend(jobs)
            jooble_total += len(jobs)
            time.sleep(0.5)
        if jooble_total == 0:
            _log("  Jooble: 0 jobs (set JOOBLE_API_KEY env var)")
        else:
            _log(f"  Jooble: {jooble_total} jobs across {len(jooble_terms)} searches")
    except Exception as e:
        _log(f"  Jooble error: {e}")

    # USAJobs
    _log("Fetching from USAJobs...")
    try:
        usajobs_terms = search_terms or ["translator", "interpreter", "photographer", "bilingual"]
        usajobs_total = 0
        for term in usajobs_terms:
            jobs = usajobs.fetch_jobs(keyword=term, location=location)
            all_jobs.extend(jobs)
            usajobs_total += len(jobs)
            time.sleep(0.5)
        if usajobs_total == 0:
            _log("  USAJobs: 0 jobs (set USAJOBS_API_KEY + USAJOBS_EMAIL env vars)")
        else:
            _log(f"  USAJobs: {usajobs_total} jobs across {len(usajobs_terms)} searches")
    except Exception as e:
        _log(f"  USAJobs error: {e}")

    # Adzuna (optional, explicit flag)
    if args.adzuna:
        _log("Fetching from Adzuna...")
        try:
            adzuna_terms = search_terms or ["translator", "photographer", "bilingual", "content creator"]
            adzuna_query = " OR ".join(adzuna_terms)
            if args.adzuna_query:
                adzuna_query = " ".join(args.adzuna_query)
            jobs = adzuna.fetch_jobs(
                query=adzuna_query,
                country=args.country,
                location=location,
            )
            if not jobs:
                _log("  Adzuna: 0 jobs (check ADZUNA_APP_ID + ADZUNA_APP_KEY env vars)")
            else:
                all_jobs.extend(jobs)
                _log(f"  Adzuna: {len(jobs)} jobs")
        except Exception as e:
            _log(f"  Adzuna error: {e}")

    return all_jobs


def deduplicate(jobs: list[dict]) -> list[dict]:
    """Remove obvious duplicates based on title + company."""
    seen = set()
    unique = []
    for job in jobs:
        key = (job["title"].lower().strip(), job["company"].lower().strip())
        if key not in seen:
            seen.add(key)
            unique.append(job)
    return unique


def truncate_descriptions(jobs: list[dict], max_len: int) -> list[dict]:
    """Truncate long descriptions to keep output manageable."""
    for job in jobs:
        desc = job.get("description", "")
        if len(desc) > max_len:
            job["description"] = desc[:max_len] + "..."
    return jobs


def main():
    parser = argparse.ArgumentParser(description="Fetch jobs from multiple APIs")
    parser.add_argument("--output", default="fetched_jobs.json", help="Output JSON file")
    parser.add_argument("--location", default="Minneapolis, MN", help="Job location (default: Minneapolis, MN)")
    parser.add_argument("--category", action="append", help="Remotive category filter (can repeat)")
    parser.add_argument("--search", action="append", help="Search terms (can repeat). Default: translator, photographer, bilingual, content creator")
    parser.add_argument("--adzuna", action="store_true", help="Include Adzuna results")
    parser.add_argument("--adzuna-query", action="append", help="Custom Adzuna query terms")
    parser.add_argument("--max-desc-len", type=int, default=2000, help="Max description length")
    parser.add_argument("--country", default="us", help="Country code for Adzuna")

    args = parser.parse_args()

    _log(f"Location: {args.location}")
    _log(f"Search terms: {args.search or '(defaults: translator, photographer, bilingual, content creator)'}")
    _log("")

    all_jobs = fetch_all(args)
    _log(f"\nTotal fetched: {len(all_jobs)}")

    all_jobs = deduplicate(all_jobs)
    _log(f"After dedup: {len(all_jobs)}")

    all_jobs = truncate_descriptions(all_jobs, args.max_desc_len)

    # Filter out jobs with empty descriptions
    all_jobs = [j for j in all_jobs if j.get("description", "").strip()]
    _log(f"After filtering empty: {len(all_jobs)}")

    output_path = Path(args.output)
    output_path.write_text(json.dumps(all_jobs, indent=2, ensure_ascii=False), encoding="utf-8")
    _log(f"\nWrote {len(all_jobs)} jobs to {output_path}")


if __name__ == "__main__":
    main()
