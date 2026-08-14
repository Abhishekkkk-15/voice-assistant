from __future__ import annotations

import argparse
from datetime import datetime
import re
import os
import sys
import ssl
import shutil
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path
from typing import Iterable
from urllib.error import URLError, HTTPError
from urllib.parse import urlparse
from urllib.request import Request, urlopen

from dotenv import load_dotenv
from mistralai.client import Mistral

load_dotenv()

OUTPUT_DEFAULT = Path("prompt/CONTEXT.md")
USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36"
UNVERIFIED_SSL_CONTEXT = ssl._create_unverified_context()


@dataclass
class ProfileData:
    name: str | None = None
    headline: str | None = None
    bio: str | None = None
    location: str | None = None
    company: str | None = None
    website: str | None = None
    email: str | None = None
    linkedin: str | None = None
    github: str | None = None
    x: str | None = None
    skills: list[str] | None = None
    projects: list[tuple[str, str]] | None = None
    experience: list[tuple[str, str]] | None = None
    about_notes: list[str] | None = None


class SimpleHTMLTextExtractor(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.chunks: list[str] = []
        self._skip_stack = 0

    def handle_starttag(self, tag: str, attrs):
        if tag in {"script", "style", "noscript"}:
            self._skip_stack += 1

    def handle_endtag(self, tag: str):
        if tag in {"script", "style", "noscript"} and self._skip_stack:
            self._skip_stack -= 1

    def handle_data(self, data: str):
        if self._skip_stack:
            return
        text = " ".join(data.split())
        if text:
            self.chunks.append(text)

    def text(self) -> str:
        return "\n".join(self.chunks)


class LinkCollector(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.links: list[tuple[str, str]] = []
        self._current_text: list[str] = []
        self._current_href: str | None = None

    def handle_starttag(self, tag: str, attrs):
        if tag == "a":
            attr_map = dict(attrs)
            self._current_href = attr_map.get("href")
            self._current_text = []

    def handle_data(self, data: str):
        if self._current_href is not None:
            text = " ".join(data.split())
            if text:
                self._current_text.append(text)

    def handle_endtag(self, tag: str):
        if tag == "a" and self._current_href:
            label = " ".join(self._current_text).strip()
            if label:
                self.links.append((label, self._current_href))
            self._current_href = None
            self._current_text = []


def fetch_url(url: str, *, verify_ssl: bool = True) -> str:
    request = Request(url, headers={"User-Agent": USER_AGENT})
    context = None if verify_ssl else UNVERIFIED_SSL_CONTEXT
    with urlopen(request, timeout=30, context=context) as response:
        raw = response.read()
        charset = response.headers.get_content_charset() or "utf-8"
        return raw.decode(charset, errors="replace")


def candidate_urls(url: str) -> list[str]:
    parsed = urlparse(url)
    if not parsed.scheme:
        return [url]

    netloc = parsed.netloc or parsed.path
    path = parsed.path if parsed.netloc else ""
    query = f"?{parsed.query}" if parsed.query else ""
    fragment = f"#{parsed.fragment}" if parsed.fragment else ""
    base_path = path.rstrip("/")

    variants = [url.rstrip("/")]
    if parsed.scheme == "https":
        variants.append(url.replace("https://", "http://", 1).rstrip("/"))
    if netloc and not netloc.startswith("www."):
        variants.append(f"{parsed.scheme}://www.{netloc}{base_path}{query}{fragment}".rstrip("/"))
        if parsed.scheme == "https":
            variants.append(f"http://www.{netloc}{base_path}{query}{fragment}".rstrip("/"))
    return list(dict.fromkeys(variants))


def fetch_best_effort(url: str) -> str:
    attempts = candidate_urls(url)
    last_error: Exception | None = None
    for attempt in attempts:
        for verify_ssl in (True, False):
            try:
                return fetch_url(attempt, verify_ssl=verify_ssl)
            except Exception as exc:  # noqa: BLE001 - best-effort public scraping
                last_error = exc
                continue
    if last_error:
        raise last_error
    raise RuntimeError(f"Unable to fetch {url}")


def clean_url(url: str) -> str:
    parsed = urlparse(url)
    if not parsed.scheme:
        return url
    return url.rstrip("/")


def extract_visible_text(html: str) -> str:
    parser = SimpleHTMLTextExtractor()
    parser.feed(html)
    return parser.text()


def extract_links(html: str) -> list[tuple[str, str]]:
    parser = LinkCollector()
    parser.feed(html)
    return parser.links


def find_first(patterns: Iterable[str], text: str) -> str | None:
    for pattern in patterns:
        match = re.search(pattern, text, flags=re.IGNORECASE | re.MULTILINE)
        if match:
            return match.group(1).strip()
    return None


def parse_github_profile(html: str, data: ProfileData) -> ProfileData:
    text = extract_visible_text(html)
    links = extract_links(html)

    data.name = data.name or find_first([r"#\s*([^\n#]+?)\s*Abhishekkkk-15", r"#\s*([^\n#]+?)\s*\n.*Abhishekkkk-15"], text)
    data.headline = data.headline or find_first([r"Software Engineer\s*·\s*([A-Za-z &-]+)", r"Building softwares?\s*that thinks?"], text)
    data.bio = data.bio or find_first([r"whoami\s*`\n\n([^`]+)", r"About Me\n\n(.+?)\n\n•"], text)
    data.github = data.github or "https://github.com/abhishekkkk-15"

    # Public GitHub page exposes the useful portfolio links and some project names.
    project_names = []
    for label, href in links:
        if "github.com/Abhishekkkk-15/" in href and label not in project_names:
            if label.lower() not in {"code", "live demo", "follow", "view my projects"}:
                project_names.append(label)
    if project_names:
        data.projects = data.projects or [(name, "Public GitHub project") for name in project_names[:8]]

    return data


def parse_portfolio_page(html: str, data: ProfileData) -> ProfileData:
    text = extract_visible_text(html)
    links = extract_links(html)

    data.name = data.name or find_first([r"Hi, I'm\s+([^\.\n]+)", r"I'm\s+([^,\n]+)"] , text)
    data.headline = data.headline or find_first([r"Software Engineer\s*·\s*([A-Za-z &-]+)", r"Building\s+software\s+that\s+thinks\."], text)
    data.bio = data.bio or find_first([r"Hi, I'm[^\n]+\n([^\n]+)", r"About Me\n\n(.+?)\n\n###"], text)
    data.location = data.location or find_first([r"([A-Za-z ]+),?\s*(India|Remote)"], text)
    data.company = data.company or find_first([r"Full Stack Developer\s*·\s*([^\n]+)", r"Where I've Worked\n\s*\d{4}\s*[—-]\s*Present\n###\s*([^\n]+)"], text)

    for label, href in links:
        href = clean_url(href)
        if href.startswith("mailto:") and not data.email:
            data.email = href.replace("mailto:", "")
        elif "linkedin.com" in href and not data.linkedin:
            data.linkedin = href
        elif "github.com/abhishekkkk-15" in href and not data.github:
            data.github = href
        elif "abhishekkkk.in" in href and not data.website:
            data.website = href
        elif href.startswith("https://x.com") or href.startswith("https://twitter.com"):
            data.x = href

    skill_keywords = [
        "TypeScript", "JavaScript", "Go", "Rust", "React", "Next.js", "Node.js", "NestJS",
        "Express", "Gin", "Socket.IO", "LangChain", "LangGraph", "Tailwind CSS", "OpenAI",
        "pgvector", "Docker", "Kubernetes", "PostgreSQL", "Redis", "MongoDB", "Kafka", "AWS",
    ]
    skills = [skill for skill in skill_keywords if re.search(rf"\b{re.escape(skill)}\b", text, flags=re.IGNORECASE)]
    if skills:
        data.skills = skills

    project_patterns = [
        (r"###\s+Jarvis Assistant\s*\n\s*(.+?)\n", "Jarvis Assistant"),
        (r"###\s+Infra-CD\s*\n\s*(.+?)\n", "Infra-CD"),
        (r"###\s+Eden\s*\n\s*(.+?)\n", "Eden"),
        (r"###\s+Collabflow\s*\n\s*(.+?)\n", "Collabflow"),
        (r"###\s+Email Scheduler\s*\n\s*(.+?)\n", "Email Scheduler"),
        (r"###\s+Internal Search\s*\n\s*(.+?)\n", "Internal Search"),
    ]
    projects: list[tuple[str, str]] = []
    for pattern, title in project_patterns:
        match = re.search(pattern, text, flags=re.IGNORECASE | re.DOTALL)
        if match:
            description = " ".join(match.group(1).split())
            projects.append((title, description))
    if projects:
        data.projects = projects

    about_notes = [
        line.strip("• ")
        for line in text.splitlines()
        if line.strip().startswith(("Systems Thinker", "AI-First Builder", "Open Source"))
    ]
    if about_notes:
        data.about_notes = about_notes

    return data


def build_context_md(data: ProfileData) -> str:
    lines: list[str] = ["# Portfolio Context", ""]
    lines.extend(["## About"])
    if data.name:
        lines.append(f"- Name: {data.name}")
    if data.github:
        lines.append(f"- GitHub handle: {data.github.rstrip('/').split('/')[-1]}")
    if data.headline:
        lines.append(f"- Role: {data.headline}")
    if data.company:
        lines.append(f"- Current role: {data.company}")
    if data.location:
        lines.append(f"- Location: {data.location}")
    if data.bio:
        lines.append(f"- Short bio: {data.bio}")
    if data.website:
        lines.append(f"- Website: {data.website}")
    if data.linkedin:
        lines.append(f"- LinkedIn: {data.linkedin}")
    if data.email:
        lines.append(f"- Email: {data.email}")
    lines.append("")

    lines.append("## What the assistant may say")
    lines.append("- Your public skills and tools")
    lines.append("- Your featured projects and their short descriptions")
    lines.append("- Your public contact links")
    lines.append("- Your work style and interests")
    lines.append("")

    lines.append("## Skills")
    if data.skills:
        for skill in data.skills:
            lines.append(f"- {skill}")
    else:
        lines.append("- Add verified skills from the public profiles")
    lines.append("")

    lines.append("## Projects")
    if data.projects:
        for title, description in data.projects[:8]:
            lines.append(f"- {title}: {description}")
    else:
        lines.append("- Add verified project names and summaries from public links")
    lines.append("")

    lines.append("## Experience")
    if data.company:
        lines.append(f"- {data.company}")
    else:
        lines.append("- Add verified work experience from the public profile")
    lines.append("")

    lines.append("## Public Links")
    if data.github:
        lines.append(f"- GitHub: {data.github}")
    if data.website:
        lines.append(f"- Portfolio: {data.website}")
    if data.linkedin:
        lines.append(f"- LinkedIn: {data.linkedin}")
    if data.x:
        lines.append(f"- X: {data.x}")
    if data.email:
        lines.append(f"- Email: {data.email}")
    lines.append("")

    lines.append("## Constraints")
    lines.append("- Do not mention private phone numbers.")
    lines.append("- Do not claim experience or credentials that are not publicly visible.")
    lines.append("- Do not present speculative information as fact.")
    lines.append("- If a question is outside this context, be honest and say so.")
    lines.append("")

    lines.append("## Voice Style")
    lines.append("- Speak like a helpful personal website guide.")
    lines.append("- Keep answers concise, direct, and easy to read aloud.")
    lines.append("- Prefer the public facts in this file over generic summaries.")

    return "\n".join(lines).rstrip() + "\n"


def gather_profile(urls: list[str]) -> ProfileData:
    data = ProfileData()
    for url in urls:
        lowered = url.lower()
        try:
            html = fetch_best_effort(url)
        except (HTTPError, URLError) as exc:
            print(f"[warn] failed to fetch {url}: {exc}", file=sys.stderr)
            continue

        if "github.com" in lowered:
            data = parse_github_profile(html, data)
        elif "abhishekkkk.in" in lowered:
            data = parse_portfolio_page(html, data)
        else:
            data = parse_portfolio_page(html, data)

    return data


def prompt_for_urls() -> list[str]:
    print("Interactive context generation")
    print("Press Enter to skip any field. Paste the public profile links you want to include.")

    urls: list[str] = []
    prompts = [
        ("GitHub", "https://github.com/your-username"),
        ("Portfolio", "https://yourportfolio.com"),
        ("LinkedIn", "https://www.linkedin.com/in/your-profile"),
        ("X", "https://x.com/your-handle"),
    ]

    for label, example in prompts:
        value = input(f"{label} URL [{example}]: ").strip()
        if value:
            urls.append(value)

    while True:
        extra = input("Add another public profile URL (or press Enter to finish): ").strip()
        if not extra:
            break
        urls.append(extra)

    return urls


def collect_source_text(url: str) -> str:
    html = fetch_best_effort(url)
    visible_text = extract_visible_text(html)
    links = extract_links(html)
    link_lines = [f"- {label}: {clean_url(href)}" for label, href in links[:60]]

    chunks = [
        f"SOURCE URL: {url}",
        "VISIBLE TEXT:",
        visible_text[:12000],
        "",
        "LINKS:",
        *link_lines,
    ]
    return "\n".join(chunks).strip()


def build_source_dossier(urls: list[str], data: ProfileData) -> str:
    pieces = [
        "You are given public profile pages for one person.",
        "Use only the facts below. Do not invent missing details.",
        "If a fact is not clearly supported, omit it or say it is not publicly specified.",
        "",
        "HEURISTIC FACTS:",
        build_context_md(data),
        "",
        "RAW SOURCES:",
    ]

    for url in urls:
        try:
            pieces.append(collect_source_text(url))
        except Exception as exc:  # noqa: BLE001 - best effort context generation
            pieces.append(f"SOURCE URL: {url}\nFETCH ERROR: {exc}")
        pieces.append("\n---\n")

    return "\n".join(pieces).strip()


def synthesize_context_with_llm(dossier: str) -> str | None:
    api_key = os.getenv("LLM_KEY")
    if not api_key:
        return None

    client = Mistral(api_key=api_key)
    response = client.chat.complete(
        model="mistral-small-latest",
        temperature=0.2,
        messages=[
            {
                "role": "system",
                "content": (
                    "You generate a reusable prompt/CONTEXT.md for a portfolio voice assistant. "
                    "Return markdown only. Use the public source dossier as the single source of truth. "
                    "Do not invent facts. Organize the file with these sections exactly: About, What the assistant may say, Skills, Projects, Experience, Public Links, Constraints, Voice Style. "
                    "If a section has no clear public data, keep a short placeholder like 'Not publicly specified'."
                ),
            },
            {"role": "user", "content": dossier},
        ],
    )

    choice = response.choices[0] if response.choices else None
    if choice is None or choice.message is None or choice.message.content is None:
        return None
    return str(choice.message.content).strip()


def create_backup_copy(path: Path) -> Path | None:
    if not path.exists():
        return None

    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    backup_path = path.with_name(f"{path.stem}.backup-{timestamp}{path.suffix}")
    shutil.copy2(path, backup_path)
    return backup_path


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate prompt/CONTEXT.md from public profile links.")
    parser.add_argument("--github", action="append", default=[], help="GitHub profile URL")
    parser.add_argument("--portfolio", action="append", default=[], help="Portfolio website URL")
    parser.add_argument("--x", action="append", default=[], help="X/Twitter profile URL")
    parser.add_argument("--linkedin", action="append", default=[], help="LinkedIn profile URL")
    parser.add_argument("--interactive", action="store_true", help="Prompt for profile links in the terminal")
    parser.add_argument("--output", default=str(OUTPUT_DEFAULT), help="Output markdown file path")
    args = parser.parse_args()

    urls = [*args.github, *args.portfolio, *args.x, *args.linkedin]
    if args.interactive or not urls:
        interactive_urls = prompt_for_urls()
        urls.extend(interactive_urls)

    if not urls:
        print("Provide at least one profile URL with --github, --portfolio, --x, or --linkedin.", file=sys.stderr)
        return 2

    data = gather_profile(urls)
    dossier = build_source_dossier(urls, data)
    synthesized = synthesize_context_with_llm(dossier)
    context_text = synthesized or build_context_md(data)
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    backup_path = create_backup_copy(output_path)
    output_path.write_text(context_text, encoding="utf-8")
    if backup_path:
        print(f"Backed up previous context to {backup_path}")
    print(f"Wrote {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
