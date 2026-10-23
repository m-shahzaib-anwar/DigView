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

## Search responsiveness source follow-up — 2026-10-23

Targeted frontend queue/search/passage tests: 14 passed. Native library tests: 5 passed, including warm worker reuse, deterministic idle cleanup, dead-worker disposal and retry after failed startup. Document-index regressions: 4 passed, 1 existing isolated-parser integration fixture ignored (requires a separately built example). Initial library test compilation exposed raw Windows handle transfer; standard OwnedHandle corrected ownership and the rerun passed. No real model/app latency, warm RAM use, UI loading behavior or process cleanup has been observed in the app for this change. No application EXE/build/installer run. User runtime review remains required; first/expired-worker searches still perform verified cold startup.

## Cross-language rental-cat query diagnostic — 2026-10-23

User reported English results matching expectations but Urdu/Roman Urdu default searches empty. A five-query local check against 26 generated demo documents reproduced the cutoff failure: expected note-042.docx scored 0.7765/rank1 in English, 0.6439/rank1 in Roman Urdu, and 0.6123/rank12 in Urdu. Default floor is 0.66; broad floor 0.55 includes 17 Roman Urdu and all 26 Urdu candidates, plus 14 and 5 candidates for two absent-topic controls. Thus the broader cutoff has poor precision here and Urdu ranking requires more than lowering a score threshold. Model/runtime hashes verified; temporary hidden local process stopped; database opened read-only and only generated-demo vectors used. No build or search-policy change. This is a small diagnostic, not held-out multilingual validation.

## Limited local Urdu/Roman Urdu assistance and bounded broader results — 2026-10-23

Verified installed-model inference over 26 generated-demo text files, 37 query rows: both assisted rental-cat queries now rank note-042.docx first at 0.7197; strict and bounded-broad policies keep only it. Original expected topics and the four multilingual guide topics survive; eight absent-topic rows return no semantic candidates with conservative fallback for unknown Urdu wording. Complete scores, prompt rewrites and fallback flags are retained in fixtures/embeddinggemma2-language-relevance.json. This is calibration over known examples, not held-out multilingual validation, full translation or perfect precision. The limited original word-alias helper is entirely local, retains exact-search input and does not change stored vectors. Unknown wording identified by Urdu hints uses literal search; other Arabic/Persian queries keep their existing model path.

Relevant suites: frontend queue/parsing/passage 14 passed; semantic core 16 passed; language helper 3 passed; desktop native library 5 passed before the final narrow Urdu-hint refinement (final reruns in STATUS.md). Coverage includes measured prompt parity/positive retention/negative controls, flat-score rejection, relative bands/cap, filter-scoped margins/counts/pagination, Unicode, negation/unknown identifiers and worker lifecycle. No app build/EXE/installer or packaged UI validation. The next user build needs no model redownload or reindex.

Final refinement reruns: language helper 3 passed, semantic 16 passed, native library 5 passed; frontend 14 passed. Total 38 relevant tests passed. Fallback requires Urdu-specific letter/grammar hints; script alone does not disable the existing model path. No application build or installer run.

## Installed DigView UI observations — 2026-10-23

Desktop controls worked on retry. In the installed Windows app: English/Urdu/Roman Urdu rental-cat lookup returned note-042.docx; expanded Urdu remained one result; English/Roman negative spacecraft queries were empty in normal/broad modes; unfamiliar Urdu used literal fallback and returned zero. Refund PDF/source snippets, extracted Word highlight, text highlight, grid/list, dark-mode divider, paused filename search and resumed parcel meaning search were observed. Loading withheld previous results. A newly created fictional ui-check-01.txt was auto-indexed; changing it from telescope to beekeeping removed the old result and made a paraphrase find the new content. Original light/grid/Overview and enabled AI were restored.

The generated one-page PDF rendered and found page1, but a real side-by-side summary/PDF layout defect shrank the page. Source correction now stacks the bounded summary above a flexible full-width PDF; 6 relevant preview/passage tests passed. No app build was run, so the corrected layout still awaits the user's new build/runtime review. Multi-page positioning, fresh-model download/cancellation/offline corruption, installer and full lifecycle coverage remain unverified. This was an installed-app smoke session over generated data, not complete regression/quality/performance validation. The additional beekeeping fixture remains in the selected demo folder.

## PDF summary layout runtime verification — 2026-10-24

The maintainer rebuilt/installed the source correction and Codex verified the real installed Windows UI. Generated record-014.pdf opens from the refund meaning query with its source summary stacked above a full-width page in light and dark themes; the former narrow side column is gone. Matching passage - page1, zoom100% to125%, scroll containment and Fit to width back to100% were observed. With AI paused and a plain filename lookup, the normal PDF preview renders at full stage width without an empty summary strip. Original light/grid/Overview and enabled AI restored. No new build, suite run or file modification by Codex; previous 6 relevant tests remain recorded separately. This validates this layout correction on one generated one-page PDF, not multi-page positioning/password handling/all PDF formats or arbitrary window sizes.

## Three-page PDF navigation — 2026-10-24

Created/visually inspected a fictional three-page PDF in the existing selected Demo root, with pantry text on page1, bicycle text on page2 and the pottery/kiln target on page3. The installed app auto-indexed it. A meaning query returned only that PDF and the preview selected page3/3, reporting that the passage continues around page3 because the winning chunk spans two pages. Observed3->2->1->2->3 navigation, correct disabled boundary buttons and no automatic jump overriding manual paging. PDF layout stayed full width.

A separate summary defect surfaced: simply taking the first320 characters of a cross-page chunk could show unrelated page2 text. Core source now chooses bounded source context near query keywords while preserving the full original passage/offsets/digest and PDF navigation; no reindex. Added Unicode/Urdu/no-anchor/leading-context regressions. Initial semantic suite18 passed and preview/passage suite6 passed; final semantic refinement outcome in STATUS.md. The summary correction is not in the installed build yet and awaits user runtime verification. Fixture remains for replay. No app build/installer; no claims about all PDF types, arbitrary page counts or generic paraphrase explanation accuracy.

## Contextual cross-page snippet runtime verification — 2026-10-24

On the maintainer's newly built/installed app, the same pottery/kiln query returns the three-page generated PDF with a corrected card snippet and preview summary containing the page3 title and pottery source context, replacing the prior unrelated bicycle text. Preview still automatically opens page3/3 and reports Passage continues around page3; full-width layout is preserved. A small adjacent generated footer remains in the bounded source snippet. This supersedes the previously pending installed-build check for the snippet refinement. The optional manual-paging repeat was stopped when the tool detected user input followed by a minimized window; prior complete navigation observations remain separate. No new app build, code change or suite run. Scope is the reproducing fixture, not generic paraphrase explanation accuracy.

## Draft review regression suites — 2026-10-24

PR review fixed counted pause/cancel fencing, persistent model-error/Retry state and Unicode-expanded casefold source offsets; added native desktop --lib tests to draft CI without application builds. Full local suites: frontend79 passed, core92 passed/4 explicit integration fixtures ignored, native desktop library8 passed (179 passing total). New tests cover overlapping interruption guards, cancellation-bit preservation, shutdown admission, paused/error state priority and Unicode source-byte offsets. Previously recorded installed-app smoke tests precede these lifecycle refinements; new native control/error behavior still needs a future user-requested runtime validation. No standalone compilation, app build or installer. Updated-head CI is checked separately before final handoff.
