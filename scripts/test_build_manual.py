"""Checks for the user manual generator; the standard library only."""
import importlib.util
import json
from pathlib import Path
import re
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("build_manual", ROOT / "scripts/build-manual.py")
build_manual = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build_manual)


class ManualTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = tempfile.TemporaryDirectory()
        cls.output = Path(cls.directory.name) / "manual"
        cls.pages = build_manual.build(cls.output)

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def text(self, page):
        return (self.output / page["file"]).read_text()

    def test_every_page_shares_the_layout_of_the_other_manuals(self):
        # The Voltkit and Battery Health Center manuals use exactly these parts.
        parts = [
            '<header class="top">', '<div class="layout">', '<aside class="side"', 'class="drawer-head"',
            '<details class="toc-m">', "<article>", '<nav class="pager"', '<aside class="toc"',
            '<p class="byline">By <a href="https://github.com/arios4k">Alexis Rios-Negron</a>',
            "Written by Alexis Rios-Negron", "Report a problem with this manual", "MIT License",
            '<script src="manual.js">', '<link rel="stylesheet" href="style.css">',
        ]
        self.assertGreaterEqual(len(self.pages), 12)
        for page in self.pages:
            text = self.text(page)
            for part in parts:
                self.assertIn(part, text, f"{page['file']} lacks {part}")
        self.assertTrue((self.output / "index.html").exists())
        for name in ("style.css", "manual.js", "search.json", ".nojekyll"):
            self.assertTrue((self.output / name).exists(), name)

    def test_the_stylesheet_and_script_are_the_shared_ones(self):
        css = (self.output / "style.css").read_text()
        for selector in (".layout", ".callout.note", ".byline", ".pager", ".group-title", ".toc-m"):
            self.assertIn(selector, css)
        self.assertIn("search.json", (self.output / "manual.js").read_text())

    def test_the_pages_the_app_links_to_exist(self):
        # The Help button opens these; renaming one would break the app.
        for name in ("index", "designer", "import", "print-center", "batch"):
            self.assertTrue((self.output / f"{name}.html").exists(), name)

    def test_every_image_is_used_and_has_alt_text(self):
        used = set()
        for page in self.pages:
            text = self.text(page)
            used |= {Path(p).name for p in re.findall(r'(?:src|srcset)="img/([^"]+)"', text)}
            for tag in re.findall(r"<img\b[^>]*>", text):
                if 'src="img/icon.svg"' in tag:
                    continue
                match = re.search(r'alt="([^"]*)"', tag)
                self.assertTrue(match and match.group(1).strip(), tag)
        shipped = {p.name for p in (ROOT / "docs/manual/img").glob("*.png")}
        self.assertEqual(shipped - used, set(), "unused images bloat the repository")

    def test_screenshots_follow_the_readers_theme(self):
        text = self.text(next(p for p in self.pages if p["file"] == "designer.html"))
        dark = 'srcset="img/designer-annotated-dark.png"'
        self.assertIn(dark, text)
        self.assertIn('src="img/designer-annotated-light.png"', text)

    def test_callouts_tables_and_steps_use_the_shared_conventions(self):
        for page in self.pages:
            text = self.text(page)
            self.assertNotRegex(text, r'class="(note|tip|warn)"')
            self.assertNotIn('class="ui"', text)
            self.assertNotIn('class="steps"', text)
            self.assertEqual(text.count("<table"), text.count('<div class="table"><table'), page["file"])

    def test_search_index_covers_every_page(self):
        index = json.loads((self.output / "search.json").read_text())
        self.assertEqual({entry["url"] for entry in index}, {p["file"] for p in self.pages})
        for entry in index:
            self.assertTrue(entry["text"] and entry["title"] and entry["group"])

    def test_the_index_lists_every_other_page(self):
        index = self.text(next(p for p in self.pages if p["file"] == "index.html"))
        for page in self.pages:
            if page["file"] != "index.html":
                self.assertIn(f'<a href="{page["file"]}">', index)

    def test_version_matches_cargo_without_needing_tomllib(self):
        import tomllib  # the test may use it; the builder must not

        expected = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
        self.assertEqual(build_manual.version(), expected)
        source = (ROOT / "scripts/build-manual.py").read_text()
        self.assertEqual(len(re.findall(r"^import tomllib|^from tomllib", source, re.M)), 0)

    def test_headings_get_unique_anchor_ids(self):
        body, heads = build_manual.convert('<h2>Same</h2><h2>Same</h2><h3 id="x">Mine</h3>')
        self.assertEqual([ident for _, ident, _ in heads], ["same", "same-2", "x"])
        self.assertIn('href="#same-2"', body)

    def test_metadata_errors_are_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            page = Path(directory) / "01-bad.html"
            page.write_text("<p>no metadata</p>")
            with self.assertRaises(build_manual.ManualError):
                build_manual.parse_page(page)
            page.write_text("<!--\ntitle: T\ngroup: Nowhere\nsummary: S\n-->")
            with self.assertRaises(build_manual.ManualError):
                build_manual.parse_page(page)

    def test_missing_images_and_missing_alt_text_fail_the_build(self):
        with self.assertRaises(build_manual.ManualError):
            build_manual.expand_shortcodes("{{figure:missing|alt|caption}}", {"other"}, "t.html", [])
        with self.assertRaises(build_manual.ManualError):
            build_manual.expand_shortcodes("{{image:other||caption}}", {"other"}, "t.html", [])


if __name__ == "__main__":
    unittest.main()
