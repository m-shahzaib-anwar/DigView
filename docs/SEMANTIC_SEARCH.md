# Optional local AI text search

Current source on `gemma-integration`, 2026-10-23. This supersedes the earlier manual Prepare text index flow and hidden Meaning toggle. Published beta artifacts are not updated by source edits.

## User flow

Choose **Download AI search** during onboarding or Settings. After the verified download finishes, DigView enables AI search, enables document-content indexing, reads text from explicitly selected folders and prepares the meaning index automatically. Setup completion and selected folders are required before background text processing starts. New/changed files picked up by the selected-root watcher are prepared automatically. The same search field combines existing exact filename/content matches with related document results; no additional toolbar button is needed. Without a model, with AI paused or while it is busy/unavailable, existing search continues.

Settings has one short purpose/privacy/size explanation and Download, Pause/Resume, Retry when needed and Remove. Further information is behind Details. Download is also consent to prepare text in selected folders; it never authorizes additional roots, image/OCR embeddings, audio or video. Scanned/encrypted/unsupported documents may be skipped.

A permanent full-width **Activity** navigation bar occupies reserved space at the bottom of the main app only. It reports library/scan/document jobs, model loading/download/indexing, OCR, active file actions and watcher/task errors. Download and AI-indexing use numeric percentages; tasks without a known total show processed counts instead of a fabricated percentage. Multiple activities remain visible in a horizontally scrollable lane. Pause AI stops AI processing and automatic document preparation; Resume continues the pipeline. Download retry may restart an incomplete transfer. Originals and the ordinary document index are preserved by model removal.

## Background scheduling

A native lifecycle loop operates independently of the window, including tray mode, and stops on app shutdown. It never downloads assets. It requires an installed, enabled model and completed setup, waits for library jobs and respects global scanning/battery suspension. It enables the document reader, waits for pending document text, then prepares pending semantic passages. Empty/failed unsupported parser records do not trigger endless extraction. Meaning-index failures surface for explicit Retry; unchanged skipped inputs wait for reconciliation/retry. A fingerprint of pending file IDs, source digests, names, modification tokens, root scan state and exclusion policy detects new work, including documents arriving during an index run. No synchronous hashing/extraction/inference runs in React.

The index freezes pending IDs per run. It counts checked/total, indexed and skipped separately; newly arriving files run in a later automatic pass. Percentages stay below 100 until successful task completion; a file-count percentage is not a time estimate. Pausing keeps completed vectors. Source revalidation, digest checks and cascading invalidation remain enforced. Exact search matches are inserted above related results before existing SQL filters/pagination are applied. Related results are approximate and can be irrelevant.

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

Vectors are 768-dimensional normalized float32 values. Google retrieval prompts are used. A document has at most 64 passages of 1200 UTF-8 bytes, with overlap; long documents therefore have partial semantic coverage. The basic implementation verifies/loads the model per operation and ranks vectors exactly; actual latency/RAM/CPU compatibility and model quality are unmeasured.

No application build or packaged/UI/model execution was run for the automatic-flow update. Relevant frontend, core orchestration/indexing and native library test results are recorded in STATUS.md. Core tests use deterministic vectors, not real weights. Validate actual optional download → automatic preparation → same-field search, restart/tray lifecycle, pause/resume, offline/corrupt-model fallback, changes during preparation, exclusion/root removal, low resources, and bottom-bar layout in the next explicitly requested build/runtime review.

Activity placement correction: onboarding has no Activity bar or reserved bottom space; the shared model controls remain available during setup.

## Relevance filtering — 2026-10-23

Semantic-only candidates now require normalized cosine similarity **>= 0.66**, applied before result totals, byte totals and pagination. Exact filename/content matches retain priority and bypass this cutoff. Filter-only queries and unavailable-model lexical fallback retain existing behavior. A successful meaning search with no passing candidates returns no results rather than filling the page with the whole index. Existing selected-root/visibility/identity/filter checks remain in force.

This provisional floor is specific to the pinned model, prompts and chunk identity. It was selected after local inference over the 14 generated, already indexed DigView Demo text files: 14 positive queries and two deliberately absent-topic controls. Every expected primary file remains above the floor; both absent-topic queries have no passing result. For `money back after ending a membership`, record-014.pdf scored 0.6971, entry-023.md 0.6700; the next demo file scored 0.6143. The floor keeps those two demo refund results. Calibration examples are not held-out validation, do not cover other library roots, and cannot guarantee recall for arbitrary real documents. One of the 14 positive queries already ranked a related secondary refund note above the expected primary PDF; filtering does not fix that ranking error.

Measured scores are preserved in `native-core/src/fixtures/embeddinggemma2-relevance.json`; the local measurement helper/raw output are under ignored `.tools/relevance-calibration`. The helper verified model/archive hashes and engine files against the pinned archive, used a temporary hidden authenticated proxy-free loopback process, read only generated demo vectors, issued fixture queries locally and stopped that engine afterward. No model downloads, app database writes or application build were performed. Regression tests cover cutoff boundaries/nonfinite values, measured examples, empty results, filtered totals/pagination and exact-match retention below the floor.

## Matching passages and optional broader results — 2026-10-23

Search responses now carry a separate match map for returned file IDs: bounded source snippet (at most 320 characters), the best passage (at most 1200 UTF-8 bytes), byte offsets and source digest. No schema change/reindex is needed: passage positions are reconstructed from the existing model/chunk identity and winning chunk number. Text replacement during ranking suppresses stale explanations. Exact text hits can select a passage around a literal term; filename-only hits without indexed text receive no invented snippet. Cards/list rows show two lines of source text and reserve matching virtualized row space. Text is rendered as ordinary React text, never HTML.

Opening a matched result revalidates selected-root/hidden/exclusion policy, source identity and full SHA-256 in a native background worker. Stale/moved/removed sources are refused with a refresh message. Text previews highlight and scroll to the matching passage. DOCX matches use a read-only extracted-text preview, not a rendered Word layout. PDFs use local PDF.js worker text to locate the passage within the first 200 pages and scroll to its text position. Anchors handle line/punctuation differences and passages split over pages. Page-location failure retains the PDF controls and explicitly says the passage could not be located; it does not claim a precise match. Manual page navigation cancels automatic positioning. The existing PDF size/password/render limits remain.

A small More related results control beside the result count explicitly lowers the cosine floor from 0.66 to 0.55 for that search. Lower-confidence results are labelled; Best matches only restores the normal floor. New search text/filter scope resets the expansion. Exact matches, scope filters, byte/result totals and pagination remain preserved in both modes. The broader floor is an exploratory option, not newly calibrated precision or a confidence probability.

Twelve newly generated English/Urdu/Roman Urdu files are under Downloads/DigView Demo/Language Checks. Their manual guide is OUTSIDE the indexed root at Downloads/DigView Language Testing Guide.md. No queries from that guide were executed. Their expected topic groups are hypotheses for later user testing, not validation results. Relevant implementation tests are recorded in STATUS.md; no app build or real-model preview UI session was run for this update.
