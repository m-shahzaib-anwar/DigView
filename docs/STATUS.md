# DigView source status

Initial public source snapshot, 2026-10-23. This repository starts with one fresh root commit; it does not include the previous development repository's history or local release archives.

See FEATURES.md for implemented scope and VALIDATION.md for observed behavior. README screenshots are real bounded Windows app captures using generated demo files. The current app is a development beta; install/upgrade and broader migration checks remain pending. No production signing or live updater is configured.

Contributors should use forks and pull requests. Contribution templates and Windows validation/secret-audit workflows are included. Keep feature implementation separate from evidence of runtime validation.

## Separate public checkout — 2026-10-23

- Active DigView working folder is now `C:\Users\User\Downloads\Apps\DigView`, with `origin` set to `https://github.com/m-shahzaib-anwar/DigView.git`; current HEAD is `1c110b1` on `main`.
- Previous development repository remains at `C:\Users\User\Downloads\Apps\File Manager` as the backup, preserving its Git metadata and all local changes. Moving that whole folder was blocked because another process is using it, so it was left intact. The existing public checkout was moved out into its own sibling folder.
- Commands: read `docs/STATUS.md`, `docs/FEATURES.md`, and `AGENTS.md`; checked remotes/HEAD; moved the public checkout with PowerShell `Move-Item`. `git status` in the relocated checkout currently reports `fatal: this operation must be run in a work tree` even though `git rev-parse --show-toplevel` resolves to this directory and `git rev-parse --is-bare-repository` returns false. Investigate this before source changes.
- No tests, builds, or packaging run. Next: resolve the relocated checkout's Git status issue, then continue work only in this DigView folder.

## Cross-platform release TODO — 2026-10-23

- Added a contributor TODO item for macOS and Linux releases, matching the existing unchecked-list style. No implementation or platform support claim is implied.
- Command: PowerShell `Add-Content` on `TODO.md` and this status log. No tests, builds, or packaging run.
- Blocker: `git status` still fails with `fatal: this operation must be run in a work tree` after the checkout move; `git rev-parse` still locates the repository at this folder. Investigate Git status before the next source change or commit.

## README roadmap checklist — 2026-10-23

- Updated README.md to list the existing Gemma, automatic-update and new macOS/Linux-release plans as unchecked roadmap items, matching TODO.md. No availability or platform validation claim added.
- Command: PowerShell text replacement in README.md and Add-Content in this log. No tests, builds, or packaging run.
- `git status` continues to fail with `fatal: this operation must be run in a work tree`; read-only `git rev-parse` resolves the working directory. Continue investigating before code changes or commits.

## Published README/TODO roadmap — 2026-10-23

- Commit `ef9bd45` pushed to `origin/main`; GitHub `ls-remote` confirmed `ef9bd452459b1add7f1a1c78ea186ac775eaf385` as the remote main head. The push output reported bypass of the pull-request rule and two required checks; the user explicitly requested this push.
- Elevated Git commands work in the relocated checkout; earlier default-shell `git status` errors came from running outside the writable workspace. Before this status note, the tree was clean and tracked `origin/main`.
- Documentation only. No tests, builds, or packaging run. Next: planned macOS/Linux releases remain unimplemented and unvalidated.

## Windows build prerequisite check — 2026-10-23

- `npm ci` dependencies were already present. Restored `src-tauri/target/.tauri/NSIS` from the backup checkout.
- Commands: verified Rustup installer SHA-256 against the official 1.29.1 binary; attempted local Rust 1.99.1 repair with Rustup, then stopped the stalled download after more than a minute without progress; ran `node scripts/build-windows.mjs --check` only. No app build, tests, or installer packaging were run.
- Result: preflight fails immediately because `.tools/toolchain/bin/cargo.exe` is missing. Rustup's local toolchain state remains incomplete, so a build cannot be verified as ready yet.
- Next: complete the Rust 1.99.1 local toolchain setup and rerun the same quick `--check`; then the user can run the actual build.

## Windows build dependencies completed — 2026-10-23

- Installed Rust 1.99.1 (MSVC x64) in a fresh Rustup slot and placed it at the documented `.tools/toolchain` path. Restored the NSIS cache from the backup checkout.
- Commands: `cargo fetch --locked --manifest-path native-core/Cargo.toml`; `cargo fetch --locked --manifest-path src-tauri/Cargo.toml`; `node scripts/build-windows.mjs --check`; offline `cargo metadata --format-version 1 --locked` for both manifests. The prerequisite check and both offline dependency-resolution checks passed.
- No application build, tests, or installer packaging were run. The user will run the build.
- Next: run `Build-Windows.cmd` from the DigView checkout when ready.
## Gemma integration branch — 2026-10-23

- Created and checked out `gemma-integration` in the public DigView checkout at `C:\Users\User\Downloads\Apps\DigView`, starting from local `main` commit `599a74a`.
- Commands: `git status --short --branch`; `git branch --list gemma-integration`; `git rev-parse --short HEAD`; `git switch -c gemma-integration`; final branch/status inspection. Existing STATUS.md changes and untracked releases/ files were preserved.
- No tests, compile checks, builds or packaging run. No commit or push. Next: implement the basic optional local Gemma integration on this branch.

## Optional EmbeddingGemma 2 text semantic search — 2026-10-23

- Implemented source on `gemma-integration` in public `Apps/DigView`: optional onboarding/Settings download, byte progress, disk-space requirement/status, cancel/retry, enable/disable/remove, explicit text embedding and Meaning-mode filtered search with visible lexical fallback. Existing search stays the default without the model. Photos/OCR-image text/audio/video are outside this path.
- Pinned ggml-org EmbeddingGemma 2 text-only BF16 GGUF revision `58faf90940886df2d62be555c8765d00dac362c5` (557950176 bytes) and llama.cpp b11835 Windows x64 CPU archive (24081155 bytes), with fixed sizes/SHA-256 and separate Apache 2.0/MIT notices. Downloads happen only on explicit user action inside the app; no weights/runtime were downloaded by this session.
- Added SQLite normalized 768-dimensional chunk vectors, bounded UTF-8 passages, document/source identity revalidation, cascaded invalidation and existing filters/pagination applied to ranked candidates. Background workers and an authenticated proxy-free loopback child keep contents local; Windows Job Objects bound lifetime/memory, and cancel/disable/exit stop the worker. Explicit reindexing is required for new/changed document text. Basic search reloads/verifies the model per operation and scans vectors exactly; no performance or hardware claim.
- Commands/work: read AGENTS.md, STATUS.md, TODO.md, FEATURES.md and integration sources; Git branch/status/diff reads; PowerShell reads of upstream Google/Hugging Face/llama.cpp documentation and pinned metadata; Python/PowerShell source/document edits; frontend source formatting. Root Cargo.lock manually references existing locked sha2/getrandom/zip packages; resolution remains unverified.
- Validation: source review only. No tests, compile checks, frontend/native builds, model execution or installer packaging run, as requested. Prior unrelated STATUS.md edits and untracked releases/ preserved. No commit/push.
- Next: user builds when ready; then explicitly authorized behavioral validation per SEMANTIC_SEARCH.md, especially real download/integrity, paraphrase retrieval, offline/no-model fallback, filters/pagination, cancellation/child cleanup, root/exclusion changes, stale-text reconciliation, Unicode/context limits and packaged Windows CPU compatibility. Published beta artifacts remain unchanged.

### Final source review after resume — 2026-10-23

- Continued from the saved branch state after interruption. Tightened cancellation between cached-asset verification and network requests, and during vector ranking/metadata revalidation; malformed/zero/nonfinite stored vectors are skipped. Search requests are serialized and obsolete pending requests coalesced.
- Read final Git/source diffs and preserved upstream license texts; formatted edited frontend files with Prettier only. No build, compile check, tests, model download/execution, commit or push. Runtime/packaged validation remains pending as described above.

## Numeric download and semantic-indexing progress — 2026-10-23

- Added numeric download percentage plus MiB and indexing percentage plus checked/total/indexed/skipped counts to the shared onboarding/Settings model panel. Byte totals include verified cached assets; 100% is gated on successful completion after integrity/install checks. Failed/cancelled work retains incomplete progress. A dedicated completion flag prevents later query status messages from turning an old failed task into 100%.
- Semantic indexing now freezes pending file IDs once per run. Newly arriving documents wait for the next run; vanished/changed/hidden/unavailable documents are checked and skipped without being counted as indexed. The denominator stays fixed, avoiding backwards percentage jumps. Empty completed runs show No pending text files. Progress is file-based, not a time estimate.
- Commands: source/instruction reads; Python edits; Prettier formatting of three changed frontend files; `npm test -- src/semantic-progress.test.ts` (3 passed); `npm run native:test -- --locked --offline semantic::tests` (3 passed, 74 filtered out). Fixtures exercise cancellation after one commit, fixed totals when files arrive/disappear mid-run, deferred new documents and no pending work on repeat. No model/network inference is exercised.
- Recovered the prior requested test run: frontend 69 passed; core 70 passed/4 ignored; desktop library 3 passed. That desktop test harness finished after interruption; it did not create a desktop application EXE or installer. Recorded the user's latest standing preference in AGENTS.md: run necessary relevant tests after changes; application EXE/build/bundle/installer commands require explicit user instruction.
- This progress update ran only the six targeted tests; no frontend/native application build, standalone compile check, installer packaging, model download/execution, commit or push. Updated SEMANTIC_SEARCH.md and VALIDATION.md. Existing unrelated working changes/releases preserved.
- Next: user-requested app build and native UI review when ready; validate real download verification/offline/cancel behavior and model-backed retrieval separately. No runtime stability or semantic quality claim is established by these deterministic tests.

## Restore search toolbar layout — 2026-10-23

- Removed the large Meaning toggle beside the search field and its button-specific CSS, restoring the previous toolbar layout. No replacement control was added; placement is deferred by user request. The dormant semantic query path remains off, and Settings/onboarding model controls and numeric progress remain available. Removed the completed-index message directing users to the hidden search entry and documented the temporary entry-point limitation.
- Commands: read instructions/status/features and toolbar sources; Python source/document edits; Prettier on App.tsx/styles.css; source diff review. No tests needed for this isolated presentation removal; no markup-mirroring tests added. No desktop EXE, app build, compile check, installer, commit or push.
- Next: agree on a semantic-search entry location before exposing the existing semantic query path again. Previously recorded progress test outcomes remain unchanged.

## Clear Gemma onboarding and Settings instructions — 2026-10-23

- Reviewed DigUp README plus Sources/DigUpApp/Onboarding.swift and shell/ui/settings.html via upstream reads before rewriting original DigView copy. Shared onboarding/Settings panel now explains purpose with a document example, supported text inputs, current hidden search-entry availability, optional download/skip path, exact transfer size, disk/CPU requirements, privacy/offline use and numbered preparation steps tied to actual labels. Clarified cancellation, retry, disable/removal and skipped progress. Moved technical model/license material into an expandable section and aligned native task messages with visible controls.
- Actions retain the same commands and conditions; labels are Download model/Retry download, Enable/Disable model, Prepare text index, Cancel current task, Remove model & text index. Regular search and the restored toolbar stay as requested. Model-backed semantic results and performance remain unvalidated.
- Commands: source/instruction reads; web/upstream README/settings/onboarding reads; Python copy/document edits; Prettier source formatting. Copy-only update: no behavioral tests needed or markup tests added. No app EXE/build, standalone compile check, installer, model download/execution, commit or push.
- Next: user reviews wording; actual packaged UI and model execution remain pending a separately requested build/validation.

## Available disk space in GB — 2026-10-23

- Shared Gemma onboarding/Settings panel displays available disk bytes as decimal GB (1 GB = 1,000,000,000 bytes), with two decimal places. Download sizes retain MiB and installation requirements retain their existing GiB units.
- Commands: instruction/status/source reads, Python display-only edit, Prettier formatting. No behavioral tests necessary for this isolated presentation change; no desktop EXE/build, compile check or packaging run.
- Next: review the new display in the next user-requested app build or preview.

## Automatic AI search setup and bottom Activity navigation — 2026-10-23

- Supersedes manual preparation and deferred search-entry decisions: optional download auto-enables the model, selected-folder document reading and semantic preparation after setup completes. Native scheduling continues in tray mode and refreshes new/changed document text from the existing selected-root watcher. No new roots or media inputs are authorized. Global scan/battery pause is respected; AI pause persists via the enabled preference and stops automatic document reading. Resume/Retry restarts orchestration; model failures surface rather than retrying continuously.
- The existing search bar now requests automatic meaning search when available, retaining exact matches above related candidates with existing filters and pagination. Missing/paused/busy/unavailable model falls back to ordinary search. No large toolbar button. Short Settings/onboarding copy replaces the long instructions; Details contains optional information. No manual indexing action is required in the normal flow.
- Added permanent full-width bottom Activity navigation in onboarding and library, with reserved layout space and raised selection toolbar. Shows library/document jobs, AI download/loading/indexing, OCR, file actions and errors; numeric percent is used for known download/semantic totals, processed counts for jobs without totals. Includes AI pause/resume. Pending-work fingerprints prevent unchanged skipped work from busy-looping while detecting documents arriving during an index run.
- Work: read AGENTS/STATUS/FEATURES and lifecycle/layout sources; Python edits and Prettier frontend formatting; existing source changes/releases preserved. Tests and final outcomes recorded below. No application EXE, frontend/native app build, standalone compile check, installer, model download/execution, commit or push.
- Next: explicitly requested build/UI and real-model validation, especially full automatic flow, resume/restart/tray, low disk/offline/corrupt models, scheduler/watch races, changes during indexing and bottom navigation layout. No performance, retrieval quality or runtime-stability claim.

### Automatic-flow validation results

- `npm test`: 72 passed across 15 frontend logic files.
- `npm run native:test -- --locked --offline semantic::`: final run 7 passed, 74 filtered out. Includes consent/busy gating, automatic text-to-embedding stages, new work, unchanged skipped work, cancellation, fixed progress totals and exact-match priority/pagination with deterministic vectors.
- `node scripts/native.mjs test --locked --offline --manifest-path src-tauri/Cargo.toml --lib`: 3 passed; only library test harness compiled/run. Native scheduling/commands are included in the harness; real model and packaged lifecycle were not exercised.
- Final source review also clears the pending-work fingerprint on cancellation so global scan/battery resume can revisit interrupted inputs. Library tests rerun for this final native adjustment; result recorded next.

- Final native library rerun after the cancellation/resume fix: 3 passed, 0 failed. Final relevant counts: frontend 72, core semantic/automation 7, desktop library 3; all passed. No app EXE or installer was built. No real-model/UI stability claim; source awaits the user's next requested build and runtime review.

## Activity bar placement correction — 2026-10-23

- User clarified Activity belongs only inside the app. Removed the onboarding ActivityBar render/import and its extra bottom padding. Main-app Activity bar and automatic AI preparation are preserved.
- Commands: source/instruction reads; Python edits; Prettier on onboarding.tsx/styles.css. Presentation-only removal: no behavioral tests necessary, no markup tests added. No app build/EXE, compile check, installer, commit or push.
- Next: review main-app Activity layout during the next explicitly requested runtime review.

## Compact desktop Activity status bar — 2026-10-23

- Replaced the oversized 76px/multiline Activity layout with a fixed 28px single-line desktop status bar. Uses 11px inline labels, small progress indicators and 22px pause/resume controls, no large heading or shadow. Workspace reserves 28px; selection controls sit 44px above the bottom. Long statuses truncate with existing full-text tooltips; multiple activities can scroll horizontally.
- Commands: instruction/status/source reads, Python CSS edit, Prettier formatting. Styling only: no behavioral tests necessary, no tests added. No EXE/app build, compiler check or installer run. Onboarding remains without an Activity bar.
- Next: visual review in the next user-requested runtime session; no visual validation claimed.

## Generated Downloads model-search demo — 2026-10-23

- Created `C:\Users\User\Downloads\DigView Demo` at the user's request, with 17 newly generated fictional fixtures: 2 embedded-text PDFs, 2 DOCX, 3 TXT, 2 Markdown, Python/TypeScript/Rust code, JSON/CSV and SVG/WAV/ZIP non-semantic category controls. Neutral filenames separate filename matching from topic retrieval. No user files copied, originals removed, code fixtures executed or app folders auto-selected.
- Added `Downloads/DigView Demo Search Guide.md` OUTSIDE the selectable demo root: 15 paraphrase/exact-match queries with expected target files and scope caveats. Expected targets are hypotheses, not observed Gemma results. Keep the guide out of the indexed selection to avoid contaminating retrieval.
- Used bundled document/PDF dependencies and authoring markers. Reopened DOCX text/OOXML and PDFs; checked PDF extractable text and generated a PDF visual contact sheet under `.tools/demo-fixture-qa`. DOCX render attempts failed because LibreOffice/soffice.exe is unavailable; Word visual layout remains unverified, files preserved as testing fixtures. No app build, tests, model execution, commit or push.
- Next: user selects only the generated Demo folder in DigView, waits for preparation, then tries queries from the separate guide. Do not claim semantic quality from fixture creation alone.

## Measured semantic relevance floor — 2026-10-23

- Diagnosed all indexed documents being returned because cosine scores were ranked without a relevance filter. Measured the installed pinned model locally against ONLY generated demo vectors: 14 readable demo files, 14 positive queries, 2 absent-topic queries. Used read-only SQLite access and a temporary hidden loopback engine with random authentication; verified weight/archive SHA-256 and executable/DLL contents against the pinned archive. No private file contents read, model downloaded or library DB changed; temporary process stopped.
- Selected provisional model-specific cosine floor 0.66 from these measurements. The membership query retains record-014.pdf (0.6971) and entry-023.md (0.6700), dropping the next demo item (0.6143) and lower. All 14 expected primary targets remain; both negative-query result lists become empty. These are calibration observations, not held-out or broader-library quality validation. A related secondary note still outranks the expected PDF on one delivery-refund query; no perfect-rank claim.
- Applied the floor before candidate counts/pagination, preserving exact filename/document matches even below the semantic floor. No-match semantic results remain empty; existing ordinary-search fallback is preserved. Updated the result notice and persisted measured synthetic score fixtures for regression coverage.
- Commands: instruction/status/source reads; local Python read-only vector metadata, checksum verification and loopback query calibration; targeted `npm run native:test -- --locked --offline semantic::` results below. No application EXE/build, standalone compile check, installer, commit or push.
- Next: user-requested app rebuild/UI review, broader held-out language/document queries and recall tuning. New model/prompt/chunk versions require fresh calibration; low-confidence relevant secondary results can be hidden by this provisional floor.

- Validation outcome: `npm run native:test -- --locked --offline semantic::` — 10 passed, 0 failed, 74 filtered out. Includes measured-score retention/no-match controls and database-level exact matches, totals, byte counts, filters and pagination. Local model calibration completed successfully; packaged app UI with this source change has not been run. No application build performed.

## Source snippets, passage previews and broader related results — 2026-10-23

- Implemented the three requested AI-search improvements: bounded actual-source snippets below filenames in cards/list rows; matching-passage highlight/scroll for text and extracted DOCX, PDF worker page/text-position location with honest fallback; explicit More related results beside result totals with Best matches only to return to strict search. No toolbar expansion or Activity sizing change.
- Native results track winning chunk, digest and safe UTF-8 byte ranges without schema change/reindex. Passage previews revalidate full content digest and selected-root/hidden/exclusion/file identity off the UI thread; changed/removed files are refused. Broader search explicitly uses 0.55 vs default 0.66; filters/counts/pagination/exact priority remain. Broader scores are not newly calibrated confidence probabilities. PDF navigation is bounded to the first 200 pages, cancellable on close/manual paging; source layout and extraction differences can prevent exact location. DOCX is extracted text, not original Word layout.
- Created 12 fictional English/Urdu/Roman Urdu files under `Downloads/DigView Demo/Language Checks`; separate `Downloads/DigView Language Testing Guide.md` stays outside the indexed folder. User deferred language retrieval testing; none of these new guide queries were run. Files may be picked up by the app's already authorized selected-root watcher if it is running.
- Commands: read AGENTS/STATUS/FEATURES and integration sources; Python edits and fixture creation; Prettier frontend formatting; affected frontend/core/native-library tests. Initial outcomes: frontend passage/preview/search logic 13 passed; core semantic suite 13 passed; native library 3 passed. Final reruns for exact-passage/relevance refinements recorded below.
- No app EXE/build, standalone compile check, installer packaging, model inference/calibration for new language files, commit or push. Existing unrelated changes/releases preserved. Next: user-requested build/UI review for snippet spacing, matching PDF positions and broader-control placement; user manually performs the separate language checks at the end.

- Final validation: `npm test -- src/match-passage.test.ts src/content-preview.test.ts src/search.test.ts` — 13 passed across 3 files; `npm run native:test -- --locked --offline semantic::` — 13 passed, 74 filtered out; `node scripts/native.mjs test --locked --offline --manifest-path src-tauri/Cargo.toml --lib` — 3 passed. Final native reruns are clean. These 29 relevant tests cover passage offset normalization, missing anchors, chunk identity, stale/removed sources, cutoff expansion and filtered pagination. Native library harness includes new commands. Actual preview UI/PDF positioning still requires a separately requested app build/runtime check; deferred multilingual retrieval queries remain unrun.

## Prepare public draft PR — 2026-10-23

- User requested a draft PR against main. Public checkout is on gemma-integration, based on main 599a74a. No pre-existing open PR for this branch was found. Preparing a source-only commit with optional local AI search, automatic preparation, compact activity bar, relevance filtering, snippets/preview navigation, tests and documentation.
- Local release checksums/build outputs and ignored tooling/demo artifacts remain outside the commit. Existing test evidence is carried into the PR; no new app build or installer is run. Latest affected suites: frontend 13, semantic core 13, native library 3 passed; broader UI, real PDF navigation and deferred language checks remain pending.
- Draft PR workflow now runs existing tests but skips frontend/application builds and artifact upload; marking ready-for-review triggers the usual full workflow. This avoids an automatic desktop EXE build merely from creating this requested draft. Manual workflow dispatch remains explicit build authorization.
- Commands: read project/status/PR template; git status/diff/log/remotes; gh authentication and open-PR lookup; remote branch checks; workflow/document edits; staged diff review. Commit/push/PR results recorded after publication. No main merge or release publication.
