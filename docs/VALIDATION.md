# Validation boundaries

The current Windows beta was built by the maintainer. The saved local build runner reported successful executable/NSIS packaging; the installer was not executed during screenshot capture.

Observed in the native executable on 2026-10-23:

- Startup reached setup without the earlier file-lock error.
- Four-step setup accepted only a generated demonstration folder and entered the library with four files.
- The query `documentation` found a generated invoice through indexed document text, although the filename did not contain that word.
- The invoice opened in the read-only text preview.
- Four app-only screenshots were saved; their provenance is in screenshots/README.md.

These observations do not establish installer upgrade/uninstall, every migration scenario, all preview formats, codec availability or cross-platform runtime support. The publication session did not rerun local compilation or tests. Pull-request validation and the explicit test commands in CONTRIBUTING.md provide separate evidence when run.

## EmbeddingGemma 2 source milestone — 2026-10-23

Optional text semantic search is implemented in `gemma-integration` source. No tests, compilation, app/model execution or packaging were run, per user instruction. No semantic quality, latency, memory or hardware compatibility result is claimed. Published beta screenshots and previous startup observations do not validate this new path. See [SEMANTIC_SEARCH.md](SEMANTIC_SEARCH.md) for the manual/behavioral validation backlog.

## Existing suites requested after Gemma source work — 2026-10-23

- `npm test`: 69 passed across 14 files.
- `npm run native:test -- --locked --offline`: 70 passed, 4 explicitly ignored integration/benchmark fixtures.
- `node scripts/native.mjs test --locked --offline --manifest-path src-tauri/Cargo.toml --lib`: 3 passed. The previously started test run finished after chat interruption; only a test harness was compiled/run, not a desktop app executable or installer.

These existing suites do not execute real EmbeddingGemma weights or establish semantic retrieval quality. Later numeric-progress coverage uses deterministic embedding fixtures and is recorded in STATUS.md.

## Automatic flow and bottom Activity — 2026-10-23

The current automatic source supersedes the earlier manual setup instructions. Frontend logic suite: 72 passed; targeted native semantic/automation suite: 7 passed (74 filtered out); final native desktop library suite: 3 passed. Core fixtures cover consent/job gating, reading before embedding, automatic refresh decisions, unchanged skipped work, cancellation/progress and exact-match priority/pagination. No model weights, native UI, installer or application EXE were run/built for this milestone. Real download-to-ready, tray/restart scheduling, battery/global pause, file changes during model work and bottom Activity layout remain runtime validation tasks.

## Local relevance calibration — 2026-10-23

Used the already installed verified model and read-only stored vectors for 14 newly generated demo text files. Ran 14 positive fixture queries and 2 absent-topic controls through a temporary local engine. A 0.66 cosine floor retained all expected primary targets and excluded all results for both absent-topic controls. The membership-refund query retained two demo refund documents; other roots were not evaluated. This is a small calibration set, not held-out quality validation. The related refund note ranked above the expected delivery-policy PDF on one query. Targeted native regression suite: 10 passed, 74 filtered out. No app executable/installer built and no packaged UI verification of the new cutoff.

## Passage snippets and broader-results source milestone — 2026-10-23

Relevant frontend passage/preview/search logic: 13 passed. Native semantic suite: 13 passed, 74 filtered out. Native desktop library: 3 passed. Coverage includes bounded best-chunk metadata, changed/removed source refusal, Unicode offsets, PDF-text normalization/anchors, explicit broader mode and filtered totals/pagination. Tests use generated deterministic fixtures; the PDF UI/page/scroll positioning and original Word layout are not visually/runtime validated. DOCX match preview intentionally displays extracted text. Twelve language fixtures were created for the user's later manual checks; none of their new model queries were run. No app executable or installer was built.
