"""Adzuna API - requires free API key from developer.adzuna.com."""

import os
import requests

BASE_URL = "https://api.adzuna.com/v1/api/jobs"

VALID_COUNTRIES = ["us", "gb", "de", "fr", "au", "nz", "ca", "in", "pl", "br", "at", "za"]


def fetch_jobs(
    query: str = "",
    country: str = "us",
    max_pages: int = 3,
    results_per_page: int = 50,
    location: str = "",
) -> list[dict]:
    """Fetch jobs from Adzuna API and normalize them.

    Requires ADZUNA_APP_ID and ADZUNA_APP_KEY environment variables.
    Returns empty list if credentials are missing.
    """
    app_id = os.environ.get("ADZUNA_APP_ID", "")
    app_key = os.environ.get("ADZUNA_APP_KEY", "")

    if not app_id or not app_key:
        return []

    if country not in VALID_COUNTRIES:
        country = "us"

    all_jobs = []
    for page in range(1, max_pages + 1):
        params = {
            "app_id": app_id,
            "app_key": app_key,
            "results_per_page": results_per_page,
            "what": query,
            "content-type": "application/json",
        }
        if location:
            params["where"] = location

        url = f"{BASE_URL}/{country}/search/{page}"
        try:
            resp = requests.get(url, params=params, timeout=30)
            resp.raise_for_status()
            data = resp.json()
            results = data.get("results", [])
            if not results:
                break
            all_jobs.extend([_normalize(job) for job in results])
        except requests.RequestException:
            break

    return all_jobs


def _normalize(job: dict) -> dict:
    """Convert Adzuna job to common format."""
    location_parts = []
    loc = job.get("location", {})
    area = loc.get("area", [])
    if area:
        location_parts = area[-2:] if len(area) >= 2 else area

    salary = ""
    sal_min = job.get("salary_min")
    sal_max = job.get("salary_max")
    if sal_min and sal_max:
        salary = f"${int(sal_min):,} - ${int(sal_max):,}"
    elif sal_min:
        salary = f"${int(sal_min):,}+"

    return {
        "title": job.get("title", ""),
        "company": job.get("company", {}).get("display_name", ""),
        "location": ", ".join(location_parts) if location_parts else "",
        "description": job.get("description", ""),
        "salary": salary,
        "url": job.get("redirect_url", ""),
        "source": "Adzuna",
        "category": job.get("category", {}).get("label", ""),
        "date_posted": job.get("created", ""),
    }
