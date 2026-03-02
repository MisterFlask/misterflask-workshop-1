"""Jooble API - free API key from jooble.org/api/about. General job aggregator, 71 countries."""

import os
import requests

API_URL_TEMPLATE = "https://jooble.org/api/{api_key}"


def fetch_jobs(
    keywords: str = "",
    location: str = "",
    max_pages: int = 3,
) -> list[dict]:
    """Fetch jobs from Jooble API and normalize them.

    Requires JOOBLE_API_KEY environment variable.
    Returns empty list if key is missing.
    """
    api_key = os.environ.get("JOOBLE_API_KEY", "")
    if not api_key:
        return []

    url = API_URL_TEMPLATE.format(api_key=api_key)

    all_jobs = []
    for page in range(1, max_pages + 1):
        payload = {
            "keywords": keywords,
            "location": location,
            "page": str(page),
        }

        try:
            resp = requests.post(url, json=payload, timeout=30)
            resp.raise_for_status()
            data = resp.json()
            jobs = data.get("jobs", [])
            if not jobs:
                break
            all_jobs.extend([_normalize(job) for job in jobs])
        except requests.RequestException:
            break

    return all_jobs


def _normalize(job: dict) -> dict:
    """Convert Jooble job to common format."""
    return {
        "title": job.get("title", ""),
        "company": job.get("company", ""),
        "location": job.get("location", ""),
        "description": job.get("snippet", ""),
        "salary": job.get("salary", ""),
        "url": job.get("link", ""),
        "source": "Jooble",
        "category": job.get("type", ""),
        "date_posted": job.get("updated", ""),
    }
