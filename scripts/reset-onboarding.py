"""Explicit developer helper: reset only setup completion, never library data."""

import csv
import io
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
from datetime import datetime, timezone


def main():
    if sys.platform != "win32":
        raise RuntimeError("This helper targets the current Windows development app.")
    # The installed app and known direct-executable names. Never terminate them.
    for image in ("digview.exe",):
        result = subprocess.run(
            ["tasklist.exe", "/FI", f"IMAGENAME eq {image}", "/FO", "CSV", "/NH"],
            capture_output=True,
            text=True,
            errors="replace",
            check=True,
            creationflags=subprocess.CREATE_NO_WINDOW,
        )
        for row in csv.reader(io.StringIO(result.stdout)):
            if row and row[0].lower() == "digview.exe":
                raise RuntimeError("DigView is running. Use tray > Quit, then retry.")
    local_data = os.environ.get("LOCALAPPDATA")
    if not local_data:
        raise RuntimeError("Windows local application-data location is unavailable.")
    database = Path(local_data) / "app.digview.desktop" / "library.sqlite3"
    if not database.is_file():
        raise RuntimeError("No existing DigView library found. A fresh app shows setup automatically.")
    # mode=rw refuses to create a new database; a short transaction touches one row.
    connection = sqlite3.connect(database.as_uri() + "?mode=rw", uri=True, timeout=3)
    try:
        connection.execute("BEGIN IMMEDIATE")
        row = connection.execute(
            "SELECT version,completed_at FROM setup_state WHERE id=1"
        ).fetchone()
        if row is None:
            raise RuntimeError("Setup state is missing. Install the current DigView build first.")
        backup = database.parent / (
            "onboarding-state-before-reset-"
            + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
            + ".json"
        )
        # Save only the previous completion marker, with no indexed filenames/content.
        with backup.open("x", encoding="utf-8") as output:
            json.dump({"version": row[0], "completed_at": row[1]}, output)
        connection.execute(
            "UPDATE setup_state SET version=0,completed_at=NULL WHERE id=1"
        )
        connection.commit()
        print("Setup status reset. Open the latest DigView build to see the steps.")
        print("Library, selected folders, indexed data and preferences were kept.")
    finally:
        connection.close()


if __name__ == "__main__":
    try:
        main()
    except (OSError, sqlite3.Error, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Cannot reset setup: {error}", file=sys.stderr)
        sys.exit(1)
