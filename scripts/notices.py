"""Collect shipped npm and locked Cargo notices without executing dependencies.

Cargo's lock includes build tools and other OS targets, so its listing is
deliberately inclusive. Run after npm install and Cargo dependency fetch.
"""
import json
import os
from pathlib import Path
import re
import tarfile
import tomllib
from notice_review import NoticeReviews

ROOT = Path(__file__).resolve().parent.parent
reviews = NoticeReviews(ROOT)
reviewed = []
parts = ["DigView - third-party notices\n\n"
         "The Cargo list includes build-time and platform-specific dependencies.\n"
         "Individual license terms follow; this collection does not replace them.\n"]
missing = []
parts.append("\nDigView application license (MIT)\n\n"
             + (ROOT / "LICENSE").read_text(encoding="utf-8")
             + "\nThird-party components (their own terms apply):\n")


def section(name, version, license_name, repository, texts, ecosystem="data", checksum=None):
    if not texts:
        texts = reviews.texts(ecosystem, name, version, license_name, checksum)
        if texts:
            reviewed.append(f"{ecosystem}: {name} {version}")
    parts.append(f"\n{'=' * 72}\n{name} {version}\nLicense: {license_name}\nSource: {repository}\n")
    if not texts:
        missing.append(f"{name} {version}: {license_name}")
    for filename, value in texts:
        parts.append(f"\n--- {filename} ---\n{value}\n")


notice = re.compile(r"^(licen[cs]e|copying|notice|copyright)([._-]|$)", re.I)
lock = json.loads((ROOT / "package-lock.json").read_text(encoding="utf-8"))
for name, info in sorted(lock["packages"].items()):
    if not name or info.get("dev"):
        continue
    folder = ROOT / name
    # Locked optional binaries for other platforms are not shipped on this machine.
    if info.get("optional") and not (folder / "package.json").is_file():
        continue
    manifest = json.loads((folder / "package.json").read_text(encoding="utf-8"))
    texts = [(p.name, p.read_text(encoding="utf-8", errors="replace"))
             for p in sorted(folder.iterdir()) if p.is_file() and notice.match(p.name)]
    repository = manifest.get("repository", "")
    if isinstance(repository, dict):
        repository = repository.get("url", "")
    section(manifest["name"], manifest["version"], manifest.get("license", "See upstream"), repository, texts, "npm", info.get("integrity"))

cargo_home = Path(os.environ.get("CARGO_HOME", ROOT / ".tools/cargo"))
cache = cargo_home / "registry/cache"
cargo = tomllib.loads((ROOT / "src-tauri/Cargo.lock").read_text(encoding="utf-8"))
for package in sorted(cargo["package"], key=lambda p: (p["name"], p["version"])):
    if not package.get("source", "").startswith("registry+"):
        continue
    name, version = package["name"], package["version"]
    archives = list(cache.glob(f"*/{name}-{version}.crate"))
    if len(archives) != 1:
        raise RuntimeError(f"Fetch Cargo package before generating notices: {name} {version}")
    with tarfile.open(archives[0], "r:gz") as archive:
        manifest_member = archive.getmember(f"{name}-{version}/Cargo.toml")
        manifest = tomllib.loads(archive.extractfile(manifest_member).read().decode("utf-8"))["package"]
        texts = []
        for member in archive.getmembers():
            if member.isfile() and member.size < 2_000_000 and notice.match(Path(member.name).name):
                texts.append((member.name.split("/", 1)[-1], archive.extractfile(member).read().decode("utf-8", errors="replace")))
    section(name, version, manifest.get("license", "See upstream"), manifest.get("repository", ""), sorted(texts), "cargo", package.get("checksum"))

upstream = ROOT / "notices/tessdata-LICENSE.txt"
if upstream.is_file():
    section("Tesseract English language model upstream", "4.0.0_best_int", "Apache-2.0",
            "https://github.com/naptha/tessdata", [(upstream.name, upstream.read_text(encoding="utf-8"))])

mpl = ROOT / "notices/MPL-SOURCES.txt"
if mpl.is_file():
    parts.append("\n" + mpl.read_text(encoding="utf-8"))
(ROOT / "docs/notice-review.txt").write_text(
    "Unresolved missing-license-text packages:\n"
    + ("\n".join(missing) if missing else "None.")
    + f"\n\n{len(reviewed)} exact-version supplemental reviews applied; provenance in notices/upstream-review.json.\n"
    + "License declarations with standard terms are distinguished from verbatim upstream notices.\n"
    + "Source availability: notices/MPL-SOURCES.txt. This is notice completeness, not legal clearance.\n", encoding="utf-8")
if missing:
    raise SystemExit(f"Refusing incomplete notice resource: {len(missing)} unresolved packages; see docs/notice-review.txt")
notice_text = "\n".join(parts).replace("\r\n", "\n").replace("\r", "\n")
(ROOT / "src-tauri/THIRD_PARTY_NOTICES.txt").write_text(notice_text, encoding="utf-8", newline="\n")
print(f"Collected notices; {len(reviewed)} supplemental reviews applied; {len(missing)} unresolved missing texts.")
