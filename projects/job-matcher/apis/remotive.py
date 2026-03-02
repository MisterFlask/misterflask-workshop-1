"""Remotive API - free, no auth required. Returns all remote job listings."""

import requests
import re

API_URL = "https://remotive.com/api/remote-jobs"

# Valid categories per Remotive docs
VALID_CATEGORIES = [
    "software-dev", "customer-support", "design", "marketing",
    "sales", "product", "business", "data", "devops", "finance",
    "human-resources", "qa", "writing", "all-others",
]


def fetch_jobs(category: str | None = None, search: str | None = None, limit: int | None = None) -> list[dict]:
    """Fetch jobs from Remotive API and normalize them."""
    params = {}
    if category and category in VALID_CATEGORIES:
        params["category"] = category
    if search:
        params["search"] = search
    if limit:
        params["limit"] = limit

    resp = requests.get(API_URL, params=params, timeout=30)
    resp.raise_for_status()
    raw_jobs = resp.json().get("jobs", [])

    return [_normalize(job) for job in raw_jobs]


def _strip_html(html: str) -> str:
    """Rough HTML tag removal."""
    text = re.sub(r"<[^>]+>", " ", html)
    text = re.sub(r"\s+", " ", text)
    return text.strip()


def _normalize(job: dict) -> dict:
    """Convert Remotive job to common format."""
    return {
        "title": job.get("title", ""),
        "company": job.get("company_name", ""),
        "location": job.get("candidate_required_location", "Anywhere"),
        "description": _strip_html(job.get("description", "")),
        "salary": job.get("salary", ""),
        "url": job.get("url", ""),
        "source": "Remotive",
        "category": job.get("category", ""),
        "date_posted": job.get("publication_date", ""),
    }
