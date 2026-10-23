# Optional local AI text search

Current source on `gemma-integration`, 2026-10-23. This supersedes the earlier manual Prepare text index flow and hidden Meaning toggle. Published beta artifacts are not updated by source edits.

## User flow

Choose **Download AI search** during onboarding or Settings. After the verified download finishes, DigView enables AI search, enables document-content indexing, reads text from explicitly selected folders and prepares the meaning index automatically. Setup completion and selected folders are required before background text processing starts. New/changed files picked up by the selected-root watcher are prepared automatically. The same search field combines existing exact filename/content matches with related document results; no additional toolbar button is needed. Without a model, with AI paused or while it is busy/unavailable, existing search continues.

Settings has one short purpose/privacy/size explanation and Download, Pause/Resume, Retry when needed and Remove. Further information is behind Details. Download is also consent to prepare text in selected folders; it never authorizes additional roots, image/OCR embeddings, audio or video. Scanned/encrypted/unsupported documents may be skipped.

A permanent full-width **Activity** navigation bar occupies reserved space at the bottom of the main app only. It reports library/scan/document jobs, model loading/download/indexing, OCR, active file actions and watcher/task errors. Download and AI-indexing use numeric percentages; tasks without a known total show processed counts instead of a fabricated percentage. Multiple activities remain visible in a horizontally scrollable lane. Pause AI stops AI processing and automatic document preparation; Resume continues the pipeline. Download retry may restart an incomplete transfer. Originals and the ordinary document index are preserved by model removal.

## Background scheduling

A native lifecycle loop operates independently of the window, including tray mode, and stops on app shutdown. It never downloads assets. It requires an installed, enabled model and completed setup, waits for library jobs and respects global scanning/battery suspension. It enables the document reader, waits for pending document text, then prepares pending semantic passages. Empty/failed unsupported parser records do not trigger endless extraction. Meaning-index failures surface for explicit Retry; unchanged skipped inputs wait for reconciliation/retry. A fingerprint of pending file IDs, source digests, names, modification tokens, root scan state and exclusion policy detects new work, including documents arriving during an index run. No synchronous hashing/extraction/inference runs in React.

The index freezes pending IDs per run. It counts checked/total, indexed and skipped separately; newly arriving files run in a later automatic pass. Percentages stay below 100 until successful task completion; a file-count percentage is not a time estimate. Pausing keeps completed vectors. Source revalidation, digest checks and cascading invalidation remain enforced. Exact search matches are inserted above related results before existing SQL filters/pagination are applied. Related results are approximate and can be irrelevant.

## Search responsiveness

While a new query is debouncing or running, the result area and totals show a loading state rather than the previous query's files. Native requests remain serialized, and intermediate queued typing requests are discarded as soon as input changes.

After verified startup, one authenticated CPU worker stays loaded across searches and indexing passes. It is released after 120 seconds without an operation (checked by the two-second scheduler), on pause/cancel/removal/shutdown, or on an operation failure. A dead worker is discarded before retry; each new process still verifies the pinned weights and extracts the verified engine archive. The last bounded query embedding is reused for filter/sort/pagination/broader-result changes; result ranking and source revalidation always run again. Query text and its vector are only retained in memory with the worker. The existing 3 GiB job limit remains; holding the model warm consumes RAM until cleanup. The first search after startup or idle cleanup still has cold-start work. Actual app latency has not been measured for this change.

## Pinned assets and licenses

| Component | Identity | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| Text weights | ggml-org/embeddinggemma-2-GGUF, revision `58faf90940886df2d62be555c8765d00dac362c5`, `embeddinggemma-2-BF16.gguf` | 557950176 | `68bae29d62fb8c7d23e98d21fd4662753ddd636e6b62b8a70dfe059a9844f216` |
| Windows x64 CPU runtime | llama.cpp `b11835`, `llama-b11835-bin-win-cpu-x64.zip` | 24081155 | `256e606f93b7f7dfb440e8d78bdbac34184c3b45ae4defead4e072a84afe63e4` |

The model is Google's **EmbeddingGemma 2**, not EmbeddingGemma 300M or the Gemini hosted API. GGUF conversion is from ggml-org, not an official Google binary. The text-only GGUF omits multimedia encoders. Model license: Apache 2.0; llama.cpp runtime: MIT. Their notices are preserved separately in `src-tauri/EMBEDDINGGEMMA_NOTICES.txt`, embedded in the executable, included as a bundle resource and copied alongside installed model files. Weights are not bundled or relicensed as DigView MIT code.

Sources checked while implementing:

- [Google model card](https://ai.google.dev/gemma/docs/embeddinggemma/model_card_2)
- [Google text embedding and retrieval prompts](https://ai.google.dev/gemma/docs/embeddinggemma/inference-embeddinggemma-with-sentence-transformers)
- [Pinned GGUF conversion](https://huggingface.co/ggml-org/embeddinggemma-2-GGUF/tree/58faf90940886df2d62be555c8765d00dac362c5)
- [Pinned llama.cpp release](https://github.com/ggml-org/llama.cpp/releases/tag/b11835)
- [llama.cpp local server API](https://github.com/ggml-org/llama.cpp/blob/b11835/tools/server/README.md)

## Limits and validation

Only Windows x64 is enabled. The text-only CPU subprocess uses two threads, authenticated proxy-free loopback requests and Windows kill-on-job-close containment. Full model/runtime SHA-256 verification remains required before executing weights/runtime. Download uses pinned HTTPS assets, bounded streamed byte counts and application-owned partial files. Model contents and queries are never uploaded. Technical/third-party notices remain separate from DigView's MIT license.

Vectors are 768-dimensional normalized float32 values. Google retrieval prompts are used. A document has at most 64 passages of 1200 UTF-8 bytes, with overlap; long documents therefore have partial semantic coverage. Each new worker verifies/loads the model, then stays warm briefly as described above. Vectors are ranked exactly; broad latency/RAM/CPU compatibility and model quality remain unvalidated.

No application build or packaged/UI/model execution was run for the automatic-flow update. Relevant frontend, core orchestration/indexing and native library test results are recorded in STATUS.md. Core tests use deterministic vectors, not real weights. Validate actual optional download → automatic preparation → same-field search, restart/tray lifecycle, pause/resume, offline/corrupt-model fallback, changes during preparation, exclusion/root removal, low resources, and bottom-bar layout in the next explicitly requested build/runtime review.

Activity placement correction: onboarding has no Activity bar or reserved bottom space; the shared model controls remain available during setup.

## Relevance filtering — 2026-10-23

The normal semantic relevance floor is normalized cosine similarity **>= 0.66**, applied before totals/pagination. The bounded leading-match rescue and broader policy below supersede a fixed cutoff alone. Exact filename/content matches retain priority and bypass this cutoff. Filter-only queries and unavailable-model lexical fallback retain existing behavior. A successful meaning search with no passing candidates returns no results rather than filling the page with the whole index. Existing selected-root/visibility/identity/filter checks remain in force.

This provisional floor is specific to the pinned model, prompts and chunk identity. It was selected after local inference over the 14 generated, already indexed DigView Demo text files: 14 positive queries and two deliberately absent-topic controls. Every expected primary file remains above the floor; both absent-topic queries have no passing result. For `money back after ending a membership`, record-014.pdf scored 0.6971, entry-023.md 0.6700; the next demo file scored 0.6143. The floor keeps those two demo refund results. Calibration examples are not held-out validation, do not cover other library roots, and cannot guarantee recall for arbitrary real documents. One of the 14 positive queries already ranked a related secondary refund note above the expected primary PDF; filtering does not fix that ranking error.

Measured scores are preserved in `native-core/src/fixtures/embeddinggemma2-relevance.json`; the local measurement helper/raw output are under ignored `.tools/relevance-calibration`. The helper verified model/archive hashes and engine files against the pinned archive, used a temporary hidden authenticated proxy-free loopback process, read only generated demo vectors, issued fixture queries locally and stopped that engine afterward. No model downloads, app database writes or application build were performed. Regression tests cover cutoff boundaries/nonfinite values, measured examples, empty results, filtered totals/pagination and exact-match retention below the floor.

## Matching passages and optional broader results — 2026-10-23

Search responses now carry a separate match map for returned file IDs: bounded source snippet (at most 320 characters), the best passage (at most 1200 UTF-8 bytes), byte offsets and source digest. No schema change/reindex is needed: passage positions are reconstructed from the existing model/chunk identity and winning chunk number. Text replacement during ranking suppresses stale explanations. Exact text hits can select a passage around a literal term; filename-only hits without indexed text receive no invented snippet. Cards/list rows show two lines of source text and reserve matching virtualized row space. Text is rendered as ordinary React text, never HTML.

Opening a matched result revalidates selected-root/hidden/exclusion policy, source identity and full SHA-256 in a native background worker. Stale/moved/removed sources are refused with a refresh message. Text previews highlight and scroll to the matching passage. DOCX matches use a read-only extracted-text preview, not a rendered Word layout. PDFs use local PDF.js worker text to locate the passage within the first 200 pages and scroll to its text position. Anchors handle line/punctuation differences and passages split over pages. Page-location failure retains the PDF controls and explicitly says the passage could not be located; it does not claim a precise match. Manual page navigation cancels automatic positioning. The existing PDF size/password/render limits remain.

A small More related results control beside the result count enables the bounded broader policy below; the old blanket 0.55 cutoff has been removed. Lower-confidence results remain labelled; Best matches only restores normal selection. New search text/filter scope resets expansion. Exact matches, scope filters, totals and pagination remain preserved. Scores are not confidence probabilities.

Twelve newly generated English/Urdu/Roman Urdu files are under Downloads/DigView Demo/Language Checks. Their manual guide is OUTSIDE the indexed root at Downloads/DigView Language Testing Guide.md. The guide queries were initially deferred; the later local language-assistance diagnostic below exercised these synthetic topics. Broad user-library/packaged validation remains pending. Relevant implementation tests are recorded in STATUS.md; no app build or real-model preview UI session was run for this update.

## Limited language assistance and bounded expansion — 2026-10-23

Before embedding a query, an original bundled Urdu/Roman Urdu keyword table can replace common search words with English keywords. It contains word aliases, not demo filenames or phrase-to-result rules. Arabic yeh/kaf variants and vowel marks normalize locally. Negation, numbers, unknown content words and filename tokens are preserved; Latin input needs at least two non-English signals, and at least half of content tokens must be known aliases. Input is bounded to 1200 UTF-8 bytes/80 tokens. This is limited vocabulary assistance, **not general translation or full multilingual support**. Original terms still drive exact search. No extra model, network request, document-vector change or reindex is needed.

Unrecognized wording with Urdu-specific letters or Urdu grammar hints uses ordinary literal search with a brief suggestion to try English or exact document words. The measured raw model placed unrelated same-language documents above the intended English document; returning these as meaning matches would be misleading. Script alone does not trigger fallback: other Arabic/Persian queries retain the existing model path. Urdu hints are conservative heuristics, not general language detection. Other unsupported Roman Urdu wording may still miss matches.

Default 0.66 candidates remain. A single leading candidate below that floor can be retained at >=0.63 only if its margin over a real finite runner-up is >=0.03. A lone candidate cannot establish separation. More related results requires a credible lead (normal or separated), an additional candidate score >=0.62 and within 0.06 of the lead, and considers at most 20 top scoped files for expansion. Normal >=0.66 candidates and exact matches are not capped. Flat weak-score lists do not expand. Lead/margin decisions are made within the user's filters before final counts/byte totals/pagination. Existing selected-root, visibility and live identity checks remain. These are provisional calibration rules, not universal confidence thresholds.

Local inference used only 26 generated demo text files with verified installed weights/runtime, read-only vectors and a temporary authenticated hidden loopback worker stopped afterward. The rental-cat target scored 0.7765 in English; both assisted Urdu/Roman Urdu prompts scored 0.7197 and ranked the target first (previously Urdu rank12/0.6123; Roman Urdu rank1/0.6439). Both modes retain only that target on this fixture. Thirty-seven query rows include original examples, the four three-language guide topics and eight absent-topic rows (two duplicated controls). Relevant target topics remain; all absent-topic rows are empty under the complete policy, including literal-only fallback for unsupported Arabic-script negatives. A few secondary topics still pass the normal floor; no perfect precision/ranking claim. These examples informed implementation and are not held-out evaluation.

Complete synthetic scores/prompts are in `native-core/src/fixtures/embeddinggemma2-language-relevance.json`; tests verify the actual Rust assistance produces the measured prompt and apply the production relevance policy to every case. Deterministic tests also cover flat-score rejection, capped expansion, unknown words/negation, Unicode, exact matches and filtered pagination. See STATUS.md for commands/outcomes. No packaged app build/UI or broad real-library language validation was performed.

Installed-app follow-up: the generated one-page refund PDF rendered and identified its matching page; Word/text highlight, tested three-language lookup, negative filtering, pause fallback/resume and new/changed-file automatic preparation were observed. A source-summary/PDF flex layout defect was corrected in source with a vertical full-width wrapper; that correction awaits the next user-requested build. See VALIDATION.md for exact scope and remaining checks.

2026-10-24 verification supersedes the pending-layout note above: the maintainer-installed build was observed with the summary above a full-width generated PDF in light/dark mode, working zoom/Fit to width, and no empty summary in ordinary paused-AI PDF previews. Matching page1 was reported. Multi-page and broader PDF compatibility remain unverified; see VALIDATION.md.

2026-10-24 multi-page follow-up: installed-app navigation was observed on a three-page generated fixture: meaning search opened the target on page3, manual Previous/Next worked through pages1-3 and boundary buttons disabled correctly. A cross-page winning chunk exposed unrelated leading summary text; source summaries now prefer bounded query-keyword context (including supported Urdu assistance) while preserving the full passage/offsets/digest used for preview location. No-anchor paraphrases retain the existing source summary. The snippet refinement requires the user's next build for runtime review; navigation itself passed in the installed app. See VALIDATION.md.

2026-10-24 snippet verification supersedes the pending-build note: the installed maintainer build now shows the reproducing three-page pottery query with page3 source context in both the card and preview summary, while retaining automatic page3 location and full-width layout. Adjacent source footer words can remain; this does not guarantee perfect sentence boundaries or all-paraphrase explanations. See VALIDATION.md.

Review follow-up: asynchronous pause/cancel cleanup now fences new model work, including overlapping controls and shutdown. Query failures persist as a Settings error with Retry and lexical fallback, avoiding repeated automatic model startup until explicit retry. Exact-passage casefold matching maps offsets to original UTF-8 source bytes. Native desktop test harness now runs on draft CI while application build/upload remain skipped. See VALIDATION.md for test evidence and pending runtime cases.
