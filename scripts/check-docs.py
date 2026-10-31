"""Docs check: keeps the flat docs/ folder consistent with the hub, llms.txt and the README.

Checks, for every docs/*.md:
- the file name matches NN-kebab-name.md (the hub README.md is exempt)
- line 1 is the breadcrumb, and the doc ends with "Related in this cluster" (hub exempt)
- the hub, llms.txt and the root README all link to it
Checks, for every Markdown file in docs/ and the repository root:
- every relative link points to a file that exists, and every #anchor to a heading in it

Exits with 1 and prints each problem when a check fails. Runs in CI and locally:
    python scripts/check-docs.py
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"
NAME_RE = re.compile(r"^[0-3][0-9]-[a-z0-9-]+\.md$")
BREADCRUMB_RE = re.compile(r"^\[Unhoard\]\(\.\./README\.md\) › \[Docs\]\(README\.md\) › ")
LINK_RE = re.compile(r"\[[^\]]*\]\(([^)\s]+)\)")


def slug(heading: str) -> str:
    """Returns the GitHub anchor for a Markdown heading."""
    text = heading.strip().lower()
    text = re.sub(r"[`*_]", "", text)
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


def anchors(path: Path) -> set[str]:
    """Returns every heading anchor in a Markdown file, outside code fences."""
    found, in_fence = set(), False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("```"):
            in_fence = not in_fence
        elif not in_fence and line.startswith("#"):
            found.add(slug(line.lstrip("#")))
    return found


def check_links(path: Path, problems: list[str]) -> None:
    """Checks that every relative link in `path` resolves to a file and an anchor."""
    text = re.sub(r"```.*?```", "", path.read_text(encoding="utf-8"), flags=re.S)
    for target in LINK_RE.findall(text):
        if re.match(r"^[a-z]+:", target):
            continue
        file_part, _, anchor = target.partition("#")
        dest = (path.parent / file_part).resolve() if file_part else path
        rel = path.relative_to(ROOT)
        if not dest.exists():
            problems.append(f"{rel}: broken link {target}")
        elif anchor and dest.suffix == ".md" and anchor not in anchors(dest):
            problems.append(f"{rel}: missing anchor {target}")


def main() -> int:
    """Runs all checks and returns the process exit code."""
    problems: list[str] = []
    hub = (DOCS / "README.md").read_text(encoding="utf-8")
    llms = (ROOT / "llms.txt").read_text(encoding="utf-8")
    readme = (ROOT / "README.md").read_text(encoding="utf-8")

    for doc in sorted(DOCS.glob("*.md")):
        lines = doc.read_text(encoding="utf-8").splitlines()
        if doc.name == "README.md":
            continue
        if not NAME_RE.match(doc.name):
            problems.append(f"docs/{doc.name}: name does not match NN-kebab-name.md")
        if not lines or not BREADCRUMB_RE.match(lines[0]):
            problems.append(f"docs/{doc.name}: line 1 is not the breadcrumb")
        if "## Related in this cluster" not in lines:
            problems.append(f"docs/{doc.name}: no 'Related in this cluster' footer")
        if f"({doc.name})" not in hub:
            problems.append(f"docs/{doc.name}: not linked from docs/README.md")
        if f"(docs/{doc.name})" not in llms:
            problems.append(f"docs/{doc.name}: not linked from llms.txt")
        if f"(docs/{doc.name})" not in readme:
            problems.append(f"docs/{doc.name}: not linked from README.md")

    for md in sorted(DOCS.glob("*.md")) + sorted(ROOT.glob("*.md")):
        check_links(md, problems)

    for problem in problems:
        print(problem)
    print(f"docs check: {len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
