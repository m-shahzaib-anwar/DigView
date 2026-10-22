import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from notice_review import NoticeReviews


class NoticeIntegrityTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parent.parent / ".tools")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / "notices").mkdir()
        (self.root / "notices/license.txt").write_text("Original upstream license text", encoding="utf-8")
        self.entry = dict(ecosystem="cargo", name="fixture", version="1.0.0", license="MIT",
                          package_sha256="archive-checksum", selected_license="MIT", reviewed_on="2026-10-21",
                          review_basis="Fixture", package_source="https://example.invalid/source.crate",
                          texts=[dict(file="notices/license.txt", source="https://example.invalid/LICENSE",
                                      sha256=hashlib.sha256(b"Original upstream license text").hexdigest())])

    def reviews(self):
        (self.root / "notices/upstream-review.json").write_text(
            json.dumps(dict(schema=1, packages=[self.entry])), encoding="utf-8")
        return NoticeReviews(self.root)

    def test_exact_package_uses_verified_text_and_provenance(self):
        result = self.reviews().texts("cargo", "fixture", "1.0.0", "MIT", "archive-checksum")
        self.assertIn("Original upstream license text", result[1][1])
        self.assertIn("License selection: MIT", result[0][1])

    def test_changed_version_does_not_reuse_review(self):
        self.assertEqual(self.reviews().texts("cargo", "fixture", "1.0.1", "MIT"), [])

    def test_changed_archive_or_declaration_is_refused(self):
        reviews = self.reviews()
        with self.assertRaisesRegex(RuntimeError, "identity changed"):
            reviews.texts("cargo", "fixture", "1.0.0", "MIT", "different-checksum")
        with self.assertRaisesRegex(RuntimeError, "declaration changed"):
            reviews.texts("cargo", "fixture", "1.0.0", "GPL-3.0")

    def test_tampered_license_is_refused(self):
        reviews = self.reviews()
        (self.root / "notices/license.txt").write_text("tampered", encoding="utf-8")
        with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
            reviews.texts("cargo", "fixture", "1.0.0", "MIT")

    def test_outside_notice_path_is_refused(self):
        self.entry["texts"][0]["file"] = "../outside.txt"
        with self.assertRaisesRegex(RuntimeError, "inside notices"):
            self.reviews().texts("cargo", "fixture", "1.0.0", "MIT")


if __name__ == "__main__":
    unittest.main()
