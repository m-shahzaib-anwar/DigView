"""Validate version-bound supplemental license evidence without network access."""
import hashlib
import json
from pathlib import Path


class NoticeReviews:
    def __init__(self, root: Path):
        self.root = root.resolve()
        self.entries = {}
        path = root / "notices/upstream-review.json"
        if not path.exists():
            return
        document = json.loads(path.read_text(encoding="utf-8"))
        if document.get("schema") != 1:
            raise RuntimeError("Unsupported supplemental notice schema")
        for entry in document["packages"]:
            key = (entry["ecosystem"], entry["name"], entry["version"])
            if key in self.entries:
                raise RuntimeError(f"Duplicate notice review: {key}")
            self.entries[key] = entry

    def texts(self, ecosystem, name, version, declared_license, checksum=None):
        entry = self.entries.get((ecosystem, name, version))
        if entry is None:
            return []
        if entry["license"] != declared_license:
            raise RuntimeError(f"License declaration changed: {name} {version}")
        if checksum and entry.get("package_sha256", entry.get("package_integrity")) != checksum:
            raise RuntimeError(f"Reviewed package identity changed: {name} {version}")
        texts = [("Review provenance", "\n".join([
            f"Reviewed: {entry['reviewed_on']}",
            f"License selection: {entry['selected_license']}",
            f"Basis: {entry['review_basis']}",
            f"Package source: {entry['package_source']}",
            "Author metadata (not an invented copyright notice): " + json.dumps(entry.get("authors", [])),
            entry.get("attribution_note", ""),
        ]))]
        for evidence in entry["texts"]:
            path = (self.root / evidence["file"]).resolve()
            if not path.is_relative_to(self.root / "notices"):
                raise RuntimeError("Supplemental notice must stay inside notices/")
            data = path.read_bytes()
            if hashlib.sha256(data).hexdigest() != evidence["sha256"]:
                raise RuntimeError(f"Supplemental notice checksum mismatch: {evidence['file']}")
            if not evidence["source"].startswith("https://"):
                raise RuntimeError("Supplemental provenance requires HTTPS")
            texts.append((evidence["file"], f"Source: {evidence['source']}\n" + data.decode("utf-8")))
        if len(texts) == 1:
            raise RuntimeError(f"Empty supplemental notice review: {name} {version}")
        return texts
