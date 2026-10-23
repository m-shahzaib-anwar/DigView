# DigView feature scope

DigView is a Windows beta for a private, user-selected file library. The current source implements:

- Explicit suggested/custom folder selection and first-run setup.
- SQLite file metadata indexing, scan reconciliation, selected-folder watching and pause/cancel controls.
- Filename/clue/filter search plus optional embedded document text indexing for supported PDF, DOCX, TXT and code files.
- Image/PDF/text and supported audio/video previews, with local OCR for selected images.
- Favourites, download-source grouping, exact duplicate review and cleanup candidates.
- Confirmed OS-trash actions with identity/containment revalidation; no permanent deletion.
- Local appearance/background preferences, local diagnostic export and optional signed updater preparation.

File contents remain on device. No app login is required. The published beta has no semantic model; unbuilt `gemma-integration` source adds optional EmbeddingGemma 2 download, automatic text preparation, same-field exact/meaning search and a persistent bottom Activity bar (see SEMANTIC_SEARCH.md). Saved-search UI is temporarily hidden. Windows migration readers preserve compatibility with earlier private builds; old identifier strings are necessary only for that import.

Windows app startup, setup, generated-folder indexing, document-text search and text preview were observed on the current beta. Installer upgrade/uninstall, broader migration scenarios and macOS/Linux runtime remain unverified. See VALIDATION.md for boundaries and README.md for screenshots.

Branch source follow-up (2026-10-23): local AI results now include bounded matching source snippets, validated passage navigation for text/extracted DOCX and best-effort PDF page/position location. More related results is an explicit lower-confidence option. Source/tests and packaged UI/model validation remain separate; see SEMANTIC_SEARCH.md.
