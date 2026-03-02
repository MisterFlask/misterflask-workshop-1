"""RemoteOK API - free, no auth required. Returns all remote job listings as JSON array."""

import requests
import re

API_URL = "https://remoteok.com/api"

HEADERS = {
    "User-Agent": "JobMatcher/1.0 (job matching tool)",
}


def fetch_jobs() -> list[dict]:
    """Fetch jobs from RemoteOK API and normalize them.

    Note: First element in the response array is a legal/metadata object, not a job.
    No server-side filtering available - returns everything.
    """
    resp = requests.get(API_URL, headers=HEADERS, timeout=30)
    resp.raise_for_status()
    raw = resp.json()

    # Skip index 0 (legal notice / metadata)
    jobs = raw[1:] if len(raw) > 1 else []

    return [_normalize(job) for job in jobs if job.get("position")]


def _strip_html(html: str) -> str:
    """Rough HTML tag removal."""
    text = re.sub(r"<[^>]+>", " ", html)
    text = re.sub(r"\s+", " ", text)
    return text.strip()


def _normalize(job: dict) -> dict:
    """Convert RemoteOK job to common format."""
    salary = ""
    sal_min = job.get("salary_min")
    sal_max = job.get("salary_max")
    if sal_min and sal_max and int(sal_min) > 0 and int(sal_max) > 0:
        salary = f"${sal_min:,} - ${sal_max:,}"
    elif sal_min and int(sal_min) > 0:
        salary = f"${sal_min:,}+"

    tags = job.get("tags", [])
    tag_str = ", ".join(tags) if tags else ""

    return {
        "title": job.get("position", ""),
        "company": job.get("company", ""),
        "location": job.get("location", "Remote"),
        "description": _strip_html(job.get("description", "")),
        "salary": salary,
        "url": f"https://remoteok.com/l/{job.get('slug', job.get('id', ''))}" if job.get("slug") or job.get("id") else job.get("url", ""),
        "source": "RemoteOK",
        "category": tag_str,
        "date_posted": job.get("date", ""),
    }
