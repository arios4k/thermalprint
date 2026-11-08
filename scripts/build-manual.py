#!/usr/bin/env python3
"""Build the ThermalPrint user manual (static HTML) from docs/manual/pages.

Standard library only, and compatible with Python 3.8 and newer. Each page is an HTML fragment that starts with a comment holding
`title`, `group`, and `summary`. Pages are ordered by their filename prefix. Shortcodes:

    {{figure:image-name|Alt text|Caption}}   a screenshot (docs/manual/images/image-name.png)
    {{version}}                              the app version from Cargo.toml

The build fails on broken internal links, missing images, or images without alt text.
"""
import argparse
import html
import json
from pathlib import Path
import re
import shutil
import sys

ROOT = Path(__file__).resolve().parent.parent
MANUAL = ROOT / "docs" / "manual"
REPOSITORY = "https://github.com/arios4k/thermalprint"
GROUP_ORDER = ["Start here", "Using ThermalPrint", "Reference", "Help"]


class ManualError(Exception):
    pass


def version() -> str:
    """The crate version. Read without tomllib so old Pythons (Ubuntu 22.04 has 3.10) work."""
    manifest = (ROOT / "Cargo.toml").read_text()
    package = re.search(r"^\[package\]\s*$(.*?)(?=^\[|\Z)", manifest, re.S | re.M)
    found = re.search(r'^version\s*=\s*"([^"]+)"', package.group(1) if package else "", re.M)
    if not found:
        raise ManualError("Cargo.toml has no [package] version")
    return found.group(1)


def slugify(text: str) -> str:
    text = re.sub(r"<[^>]+>", "", html.unescape(text)).lower()
    return re.sub(r"[^a-z0-9]+", "-", text).strip("-") or "section"


def parse_page(path: Path) -> dict:
    source = path.read_text()
    match = re.match(r"\s*<!--(.*?)-->\s*", source, re.S)
    if not match:
        raise ManualError(f"{path.name}: missing metadata comment")
    meta = dict(
        (key.strip(), value.strip())
        for key, _, value in (line.partition(":") for line in match.group(1).strip().splitlines())
    )
    for key in ("title", "group", "summary"):
        if not meta.get(key):
            raise ManualError(f"{path.name}: metadata is missing '{key}'")
    if meta["group"] not in GROUP_ORDER:
        raise ManualError(f"{path.name}: unknown group {meta['group']!r}")
    stem = re.sub(r"^\d+-", "", path.stem)
    return {**meta, "slug": stem, "file": f"{stem}.html", "body": source[match.end():]}


def expand_shortcodes(body: str, images: set, page: str) -> str:
    def figure(match):
        name, alt, caption = (part.strip() for part in match.group(1).split("|", 2))
        if name not in images:
            raise ManualError(f"{page}: unknown image {name!r}")
        if not alt:
            raise ManualError(f"{page}: image {name!r} needs alt text")
        return (
            f'<figure><a href="images/{name}.png" target="_blank" rel="noopener">'
            f'<img src="images/{name}.png" alt="{html.escape(alt, quote=True)}" loading="lazy"></a>'
            f"<figcaption>{caption}</figcaption></figure>"
        )

    body = re.sub(r"\{\{figure:(.*?)\}\}", figure, body)
    return body.replace("{{version}}", version())


def add_heading_ids(body: str, page: str):
    """Give every h2/h3 an id and return (body, table of contents entries)."""
    seen, toc = set(), []

    def heading(match):
        level, attrs, inner = match.group(1), match.group(2), match.group(3)
        found = re.search(r'id="([^"]+)"', attrs)
        slug = found.group(1) if found else slugify(inner)
        base, count = slug, 2
        while slug in seen:
            slug, count = f"{base}-{count}", count + 1
        seen.add(slug)
        toc.append((int(level), slug, re.sub(r"<[^>]+>", "", inner)))
        attrs = attrs if found else f'{attrs} id="{slug}"'
        return f'<h{level}{attrs}>{inner} <a class="anchor" href="#{slug}" aria-label="Link to this section">#</a></h{level}>'

    return re.sub(r"<h([23])([^>]*)>(.*?)</h\1>", heading, body, flags=re.S), toc


def plain_text(body: str) -> str:
    text = re.sub(r"<(script|style)[^>]*>.*?</\1>", " ", body, flags=re.S)
    return re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]+>", " ", text))).strip()


def render_nav(pages, current):
    out = []
    for group in GROUP_ORDER:
        members = [p for p in pages if p["group"] == group]
        if not members:
            continue
        out.append(f'<div class="nav-group"><p class="nav-title">{html.escape(group)}</p><ul>')
        for page in members:
            active = ' aria-current="page"' if page["slug"] == current else ""
            out.append(f'<li><a href="{page["file"]}"{active}>{html.escape(page["title"])}</a></li>')
        out.append("</ul></div>")
    return "\n".join(out)


def render_page(template, pages, index, body, toc):
    page = pages[index]
    toc_html = ""
    if len(toc) > 1:
        items = "".join(
            f'<li class="l{level}"><a href="#{slug}">{html.escape(text)}</a></li>' for level, slug, text in toc
        )
        toc_html = f'<nav class="toc" aria-label="On this page"><p>On this page</p><ul>{items}</ul></nav>'
    previous = pages[index - 1] if index else None
    following = pages[index + 1] if index + 1 < len(pages) else None
    pager = '<nav class="pager" aria-label="Previous and next page">'
    pager += (
        f'<a class="prev" href="{previous["file"]}"><span>Previous</span>{html.escape(previous["title"])}</a>'
        if previous
        else "<span></span>"
    )
    pager += (
        f'<a class="next" href="{following["file"]}"><span>Next</span>{html.escape(following["title"])}</a>'
        if following
        else "<span></span>"
    )
    pager += "</nav>"
    replacements = {
        "{{title}}": html.escape(page["title"]),
        "{{summary}}": html.escape(page["summary"], quote=True),
        "{{nav}}": render_nav(pages, page["slug"]),
        "{{toc}}": toc_html,
        "{{content}}": body,
        "{{pager}}": pager,
        "{{version}}": version(),
        "{{repository}}": REPOSITORY,
        "{{home}}": "index.html",
    }
    result = template
    for key, value in replacements.items():
        result = result.replace(key, value)
    return result


def check_links(output: Path, pages) -> None:
    files = {p["file"] for p in pages}
    for page in pages:
        text = (output / page["file"]).read_text()
        for href in re.findall(r'(?:href|src)="([^"]+)"', text):
            if re.match(r"(https?:|mailto:|#|data:)", href):
                continue
            target, _, fragment = href.partition("#")
            if not target:
                continue
            if target in files:
                if fragment and f'id="{fragment}"' not in (output / target).read_text():
                    raise ManualError(f"{page['file']}: link to missing section {href}")
            elif not (output / target).exists():
                raise ManualError(f"{page['file']}: broken link {href}")
        for tag in re.findall(r"<img\b[^>]*>", text):
            if 'alt="' not in tag:
                raise ManualError(f"{page['file']}: image without alt text: {tag[:80]}")


def build(output: Path) -> list:
    sources = sorted((MANUAL / "pages").glob("*.html"))
    if not sources:
        raise ManualError("No pages found in docs/manual/pages")
    images = {p.stem for p in (MANUAL / "images").glob("*.png")}
    pages = [parse_page(p) for p in sources]
    pages.sort(key=lambda p: (GROUP_ORDER.index(p["group"]), p["file"] != "index.html"))
    # Keep the numeric order inside each group: files were sorted by name above.
    order = {p["file"]: i for i, p in enumerate(parse_page(s) for s in sources)}
    pages.sort(key=lambda p: (GROUP_ORDER.index(p["group"]), order[p["file"]]))
    template = (MANUAL / "template.html").read_text()
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    shutil.copytree(MANUAL / "assets", output / "assets")
    shutil.copytree(MANUAL / "images", output / "images")
    index_entries = []
    for number, page in enumerate(pages):
        body = expand_shortcodes(page["body"], images, page["file"])
        body, toc = add_heading_ids(body, page["file"])
        (output / page["file"]).write_text(render_page(template, pages, number, body, toc))
        index_entries.append(
            {
                "title": page["title"],
                "url": page["file"],
                "summary": page["summary"],
                "headings": [text for _, _, text in toc],
                "text": plain_text(body)[:6000],
            }
        )
    (output / "assets" / "search-index.json").write_text(json.dumps(index_entries, ensure_ascii=False))
    (output / ".nojekyll").touch()
    check_links(output, pages)
    return pages


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output", type=Path, default=ROOT / "target" / "manual")
    args = parser.parse_args()
    try:
        pages = build(args.output)
    except ManualError as error:
        print(f"Manual build failed: {error}", file=sys.stderr)
        return 1
    print(f"Built {len(pages)} pages into {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
