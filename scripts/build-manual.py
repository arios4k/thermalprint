#!/usr/bin/env python3
"""Build the ThermalPrint user manual (static HTML) from docs/manual/pages.

The page layout, stylesheet, and search script are the same as the Voltkit and Battery
Health Center manuals, so the three read as one family. Standard library only, and
compatible with Python 3.8 and newer (the APT release job runs Python 3.10).

Each page is an HTML fragment that starts with a comment holding `title`, `group`, and
`summary`; pages are ordered by filename. Shortcodes:

    {{figure:name|Alt text|Caption}}   a screenshot that follows the reader's light or dark
                                       theme (img/name-light.png and img/name-dark.png)
    {{image:name|Alt text|Caption}}    one picture for both themes (img/name.png)
    {{pages-table}}                    a table of every page with its summary

The build fails on broken links, missing images, or images without alt text.
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
SITE_TITLE = "ThermalPrint Manual"
REPOSITORY = "https://github.com/arios4k/thermalprint"
AUTHOR = "https://github.com/arios4k"
GROUP_ORDER = ["Start here", "The app", "Settings and data", "Reference"]

PAGE = """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>@TITLE@</title>
<meta name="description" content="@DESC@">
<meta name="author" content="Alexis Rios-Negron">
<link rel="icon" href="img/icon.svg"><link rel="stylesheet" href="style.css"></head>
<body>
<header class="top"><button class="menu" aria-label="Open the menu" aria-expanded="false" aria-controls="nav">☰</button>
<a class="brand" href="index.html"><img src="img/icon.svg" alt="" width="28" height="28"> <span>@SITE@</span></a>
<button class="find" aria-label="Search the manual" aria-expanded="false">⌕</button>
<input id="q" type="search" placeholder="Search the manual…" aria-label="Search the manual" autocomplete="off">
<a class="ghost" href="@REPO@">GitHub</a></header>
<div id="results" class="results" hidden></div>
<div class="layout">
<aside class="side" id="nav" aria-label="Manual pages"><div class="drawer-head"><a class="brand" href="index.html"><img src="img/icon.svg" alt="" width="28" height="28"> <span>@SITE@</span></a><button class="close" aria-label="Close the menu">✕ Close</button></div>@NAV@</aside>
<main><details class="toc-m"><summary>On this page</summary><ul>@TOC@</ul></details><article>@BODY@</article>@PAGER@
<footer>Written by Alexis Rios-Negron · ThermalPrint @VERSION@ · <a href="@REPO@/issues">Report a problem with this manual</a> · MIT License</footer></main>
<aside class="toc" aria-label="On this page"><div class="group-title">On this page</div><ul>@TOC@</ul></aside>
</div><script src="manual.js"></script></body></html>
"""


class ManualError(Exception):
    pass


def esc(text: str) -> str:
    return html.escape(text, quote=True)


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


def expand_shortcodes(body: str, images: set, page: str, pages: list) -> str:
    def parts(raw):
        name, alt, caption = (part.strip() for part in raw.split("|", 2))
        if not alt:
            raise ManualError(f"{page}: image {name!r} needs alt text")
        return name, alt, caption

    def themed(match):
        name, alt, caption = parts(match.group(1))
        for theme in ("light", "dark"):
            if f"{name}-{theme}" not in images:
                raise ManualError(f"{page}: missing image img/{name}-{theme}.png")
        return (
            "<figure><picture>"
            f'<source media="(prefers-color-scheme: dark)" srcset="img/{name}-dark.png">'
            f'<img src="img/{name}-light.png" alt="{esc(alt)}" loading="lazy">'
            f"</picture><figcaption>{caption}</figcaption></figure>"
        )

    def single(match):
        name, alt, caption = parts(match.group(1))
        if name not in images:
            raise ManualError(f"{page}: missing image img/{name}.png")
        return f'<figure><img src="img/{name}.png" alt="{esc(alt)}" loading="lazy"><figcaption>{caption}</figcaption></figure>'

    def table(_):
        rows = "".join(
            f'<tr><td><a href="{p["file"]}">{esc(p["title"])}</a></td><td>{esc(p["summary"])}</td></tr>'
            for p in pages
            if p["slug"] != "index"
        )
        return f'<div class="table"><table><thead><tr><th>Page</th><th>What it covers</th></tr></thead><tbody>{rows}</tbody></table></div>'

    body = re.sub(r"\{\{figure:(.*?)\}\}", themed, body)
    body = re.sub(r"\{\{image:(.*?)\}\}", single, body)
    return re.sub(r"\{\{pages-table\}\}", table, body)


def convert(body: str):
    """Give headings ids and anchors, wrap tables, and return (html, headings)."""
    body = re.sub(r"<table\b", '<div class="table"><table', body)
    body = body.replace("</table>", "</table></div>")
    # Tables written by the pages-table shortcode are already wrapped.
    body = body.replace('<div class="table"><div class="table">', '<div class="table">').replace("</table></div></div>", "</table></div>")
    heads, seen = [], set()

    def heading(match):
        level, attrs, text = match.group(1), match.group(2), match.group(3)
        found = re.search(r'id="([^"]+)"', attrs)
        ident = found.group(1) if found else slugify(text)
        while ident in seen:
            ident += "-2"
        seen.add(ident)
        if level in "23":
            heads.append((int(level), ident, re.sub(r"<[^>]+>", "", text)))
        anchor = f'<a class="anchor" href="#{ident}" aria-label="Link to this section">#</a>' if level != "1" else ""
        return f'<h{level} id="{ident}">{text}{anchor}</h{level}>'

    return re.sub(r"<h([1-3])([^>]*)>(.*?)</h\1>", heading, body, flags=re.S), heads


def build(output: Path) -> list:
    sources = sorted((MANUAL / "pages").glob("*.html"))
    if not sources:
        raise ManualError("No pages found in docs/manual/pages")
    parsed = [parse_page(p) for p in sources]
    order = {p["file"]: n for n, p in enumerate(parsed)}
    pages = sorted(parsed, key=lambda p: (GROUP_ORDER.index(p["group"]), order[p["file"]]))
    images = {p.stem for p in (MANUAL / "img").glob("*.png")}
    current = version()

    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    shutil.copytree(MANUAL / "img", output / "img")
    for name in ("style.css", "manual.js"):
        shutil.copy2(MANUAL / name, output / name)

    for page in pages:
        body = expand_shortcodes(page["body"], images, page["file"], pages)
        page["body"], page["heads"] = convert(f"<h1>{esc(page['title'])}</h1>" + body)
        plain = re.sub(r'<a class="anchor"[^>]*>#</a>', "", page["body"])
        page["text"] = re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]+>", " ", plain))).strip()

    def nav(active):
        out = ""
        for group in GROUP_ORDER:
            members = [p for p in pages if p["group"] == group]
            if not members:
                continue
            out += f'<div class="group"><div class="group-title">{esc(group)}</div><ul>'
            for p in members:
                cur = ' aria-current="page"' if p["slug"] == active else ""
                out += f'<li><a href="{p["file"]}"{cur}>{esc(p["title"])}</a></li>'
            out += "</ul></div>"
        return out

    (output / "search.json").write_text(
        json.dumps(
            [
                dict(
                    url=p["file"],
                    title=p["title"],
                    group=p["group"],
                    text=p["text"][:6000],
                    heads=[dict(id=i, t=t) for _, i, t in p["heads"]],
                )
                for p in pages
            ],
            ensure_ascii=False,
        )
    )

    byline = f'<p class="byline">By <a href="{AUTHOR}">Alexis Rios-Negron</a> · ThermalPrint {current}</p>'
    for number, page in enumerate(pages):
        toc = "".join(f'<li class="l{level}"><a href="#{ident}">{esc(text)}</a></li>' for level, ident, text in page["heads"])
        previous = pages[number - 1] if number else None
        following = pages[number + 1] if number + 1 < len(pages) else None
        pager = '<nav class="pager" aria-label="Previous and next page">'
        pager += (
            f'<a class="prev" href="{previous["file"]}"><span>Previous</span>{esc(previous["title"])}</a>'
            if previous
            else "<span></span>"
        )
        pager += (
            f'<a class="next" href="{following["file"]}"><span>Next</span>{esc(following["title"])}</a>'
            if following
            else "<span></span>"
        )
        pager += "</nav>"
        title = SITE_TITLE if page["title"] == SITE_TITLE else f"{page['title']} · {SITE_TITLE}"
        text = PAGE
        for key, value in {
            "@TITLE@": esc(title),
            "@SITE@": SITE_TITLE,
            "@DESC@": esc(page["summary"]),
            "@REPO@": REPOSITORY,
            "@NAV@": nav(page["slug"]),
            "@TOC@": toc,
            "@PAGER@": pager,
            "@VERSION@": current,
        }.items():
            text = text.replace(key, value)
        body = page["body"].replace("</h1>", "</h1>" + byline, 1)
        (output / page["file"]).write_text(text.replace("@BODY@", body))
    (output / ".nojekyll").touch()
    check(output, pages)
    return pages


def check(output: Path, pages: list) -> None:
    """A broken link, missing image, or image without alt text fails the build."""
    ids = {p["file"]: set(re.findall(r'id="([^"]+)"', (output / p["file"]).read_text())) for p in pages}
    bad = []
    for page in pages:
        text = (output / page["file"]).read_text()
        for href in re.findall(r'href="([^"]+)"', text):
            if re.match(r"(https?:|mailto:|data:)", href):
                continue
            target, _, fragment = href.partition("#")
            target = target or page["file"]
            if target in ids:
                if fragment and fragment not in ids[target]:
                    bad.append(f"{page['file']}: no section {href}")
            elif not (output / target).exists():
                bad.append(f"{page['file']}: no page {href}")
        for source in re.findall(r'(?:src|srcset)="([^"]+)"', text):
            if not (output / source).exists():
                bad.append(f"{page['file']}: missing {source}")
        for tag in re.findall(r"<img\b[^>]*>", text):
            if 'alt="' not in tag:
                bad.append(f"{page['file']}: image without alt text: {tag[:70]}")
    if bad:
        raise ManualError("Broken links or images in the manual:\n  " + "\n  ".join(bad))


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
