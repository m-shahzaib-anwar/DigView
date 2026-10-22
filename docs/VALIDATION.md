# Validation boundaries

The current Windows beta was built by the maintainer. The saved local build runner reported successful executable/NSIS packaging; the installer was not executed during screenshot capture.

Observed in the native executable on 2026-10-23:

- Startup reached setup without the earlier file-lock error.
- Four-step setup accepted only a generated demonstration folder and entered the library with four files.
- The query `documentation` found a generated invoice through indexed document text, although the filename did not contain that word.
- The invoice opened in the read-only text preview.
- Four app-only screenshots were saved; their provenance is in screenshots/README.md.

These observations do not establish installer upgrade/uninstall, every migration scenario, all preview formats, codec availability or cross-platform runtime support. The publication session did not rerun local compilation or tests. Pull-request validation and the explicit test commands in CONTRIBUTING.md provide separate evidence when run.
