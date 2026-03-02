"""USAJobs API - free API key from developer.usajobs.gov. US federal government jobs."""

import os
import requests

BASE_URL = "https://data.usajobs.gov/api/search"


def fetch_jobs(
    keyword: str = "",
    location: str = "",
    results_per_page: int = 50,
    max_pages: int = 2,
) -> list[dict]:
    """Fetch jobs from USAJobs API and normalize them.

    Requires USAJOBS_API_KEY and USAJOBS_EMAIL environment variables.
    Returns empty list if credentials are missing.
    """
    api_key = os.environ.get("USAJOBS_API_KEY", "")
    email = os.environ.get("USAJOBS_EMAIL", "")

    if not api_key or not email:
        return []

    headers = {
        "Authorization-Key": api_key,
        "User-Agent": email,
        "Host": "data.usajobs.gov",
    }

    all_jobs = []
    for page in range(1, max_pages + 1):
        params = {
            "ResultsPerPage": results_per_page,
            "Page": page,
        }
        if keyword:
            params["Keyword"] = keyword
        if location:
            params["LocationName"] = location

        try:
            resp = requests.get(BASE_URL, headers=headers, params=params, timeout=30)
            resp.raise_for_status()
            data = resp.json()
            results = data.get("SearchResult", {}).get("SearchResultItems", [])
            if not results:
                break
            all_jobs.extend([_normalize(item) for item in results])
        except requests.RequestException:
            break

    return all_jobs


def _normalize(item: dict) -> dict:
    """Convert USAJobs result to common format."""
    job = item.get("MatchedObjectDescriptor", {})

    # Location - USAJobs can have multiple
    locations = job.get("PositionLocation", [])
    loc_strs = [f"{l.get('CityName', '')}, {l.get('CountrySubDivisionCode', '')}" for l in locations]
    location = "; ".join(loc_strs) if loc_strs else ""

    # Salary
    salary = ""
    remuneration = job.get("PositionRemuneration", [])
    if remuneration:
        r = remuneration[0]
        sal_min = r.get("MinimumRange", "")
        sal_max = r.get("MaximumRange", "")
        interval = r.get("Description", "")
        if sal_min and sal_max:
            salary = f"${sal_min} - ${sal_max} {interval}"

    # Description - combine qualification summary + user area
    desc_parts = []
    qual = job.get("QualificationSummary", "")
    if qual:
        desc_parts.append(qual)
    user_area = job.get("UserArea", {}).get("Details", {})
    major_duties = user_area.get("MajorDuties", "")
    if major_duties:
        if isinstance(major_duties, list):
            desc_parts.extend(major_duties)
        else:
            desc_parts.append(major_duties)

    # Apply URL
    apply_url = job.get("ApplyURI", [""])[0] if job.get("ApplyURI") else job.get("PositionURI", "")

    return {
        "title": job.get("PositionTitle", ""),
        "company": job.get("OrganizationName", ""),
        "location": location,
        "description": " ".join(desc_parts),
        "salary": salary,
        "url": apply_url,
        "source": "USAJobs",
        "category": job.get("JobCategory", [{}])[0].get("Name", "") if job.get("JobCategory") else "",
        "date_posted": job.get("PublicationStartDate", ""),
    }
