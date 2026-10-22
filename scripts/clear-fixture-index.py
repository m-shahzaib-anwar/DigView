"""Reset only the exact generated smoke-test root after closing DigView.

Does not delete any original file, cache, database or other selected root.
"""
import os
from pathlib import Path
import sqlite3

workspace = Path(__file__).resolve().parent.parent
fixture = (workspace / ".tools/e2e-library").resolve()
database = Path(os.environ["LOCALAPPDATA"]) / "app.digview.desktop/library.sqlite3"
connection = sqlite3.connect(database.as_uri() + "?mode=rw", uri=True)
connection.execute("PRAGMA foreign_keys=ON")
def normalized(path):
    return os.path.normcase(str(Path(path).resolve()).removeprefix("\\\\?\\"))


matching = [(i, p) for i, p in connection.execute("SELECT id,path FROM roots")
            if normalized(p) == normalized(fixture)]
if len(matching) != 1:
    raise RuntimeError("Expected exactly one generated fixture root; nothing changed")
before = sorted(p.name for p in fixture.iterdir() if p.is_file())
with connection:
    connection.execute("DELETE FROM roots WHERE id=? AND path=?", matching[0])
assert before == sorted(p.name for p in fixture.iterdir() if p.is_file())
print("Removed generated root from the index; all original fixture files preserved.")
