"""Checks for the user manual generator; the standard library only."""
import importlib.util
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

    def test_every_page_is_built_with_navigation_and_a_title(self):
        self.assertGreaterEqual(len(self.pages), 12)
        for page in self.pages:
            text = (self.output / page["file"]).read_text()
            self.assertIn("<h1>", text)
            self.assertIn('aria-label="Manual pages"', text)
            self.assertIn(page["title"].replace("&", "&amp;"), text)
        self.assertTrue((self.output / "index.html").exists())
        self.assertTrue((self.output / ".nojekyll").exists())

    def test_the_pages_the_app_links_to_exist(self):
        # The Help button opens these; renaming one would break the app.
        for name in ("index", "designer", "import", "print-center", "batch"):
            self.assertTrue((self.output / f"{name}.html").exists(), name)

    def test_every_image_is_used_and_every_used_image_has_alt_text(self):
        used = set()
        for page in self.pages:
            text = (self.output / page["file"]).read_text()
            used |= set(re.findall(r'src="images/([^"]+)"', text))
            for tag in re.findall(r"<img\b[^>]*>", text):
                match = re.search(r'alt="([^"]*)"', tag)
                if "assets/icon.svg" in tag:
                    continue
                self.assertTrue(match and match.group(1).strip(), tag)
        shipped = {p.name for p in (ROOT / "docs/manual/images").glob("*.png")}
        self.assertEqual(shipped - used, set(), "unused images bloat the repository")

    def test_search_index_covers_every_page(self):
        import json

        index = json.loads((self.output / "assets/search-index.json").read_text())
        self.assertEqual({entry["url"] for entry in index}, {p["file"] for p in self.pages})
        self.assertTrue(all(entry["text"] for entry in index))

    def test_headings_get_unique_anchor_ids(self):
        body, toc = build_manual.add_heading_ids("<h2>Same</h2><h2>Same</h2><h3 id=\"x\">Mine</h3>", "t")
        ids = [slug for _, slug, _ in toc]
        self.assertEqual(ids, ["same", "same-2", "x"])
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

    def test_unknown_images_fail_the_build(self):
        with self.assertRaises(build_manual.ManualError):
            build_manual.expand_shortcodes("{{figure:missing|alt|caption}}", {"other"}, "t.html")


if __name__ == "__main__":
    unittest.main()
