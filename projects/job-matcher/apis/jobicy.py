"""Jobicy API - free, no auth required. Remote jobs across categories including creative/marketing."""

import requests

API_URL = "https://jobicy.com/api/v2/remote-jobs"


def fetch_jobs(
    count: int = 50,
    industry: str = "",
    tag: str = "",
) -> list[dict]:
    """Fetch jobs from Jobicy API and normalize them.

    No auth required. Max 50 results per request.
    Industries: marketing, design-multimedia, copywriting, business, customer-support, etc.
    """
    params = {"count": min(count, 50)}
    if industry:
        params["industry"] = industry
    if tag:
        params["tag"] = tag

    headers = {"User-Agent": "JobMatcher/1.0 (personal job matching tool)"}
    resp = requests.get(API_URL, params=params, headers=headers, timeout=30)
    resp.raise_for_status()
    data = resp.json()
    jobs = data.get("jobs", [])

    return [_normalize(job) for job in jobs if job.get("jobTitle")]


def _normalize(job: dict) -> dict:
    """Convert Jobicy job to common format."""
    # Salary - Jobicy has min/max fields
    salary = ""
    sal_min = job.get("annualSalaryMin")
    sal_max = job.get("annualSalaryMax")
    currency = job.get("salaryCurrency", "USD")
    if sal_min and sal_max:
        salary = f"{currency} {sal_min} - {sal_max}"
    elif sal_min:
        salary = f"{currency} {sal_min}+"

    # Location
    geo = job.get("jobGeo", "")
    location = geo if geo else "Remote"

    return {
        "title": job.get("jobTitle", ""),
        "company": job.get("companyName", ""),
        "location": location,
        "description": job.get("jobExcerpt", ""),
        "salary": salary,
        "url": job.get("url", ""),
        "source": "Jobicy",
        "category": job.get("jobIndustry", [""])[0] if job.get("jobIndustry") else "",
        "date_posted": job.get("pubDate", ""),
    }
