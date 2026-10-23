# Contributor TODO

Planned work below is not available in the current beta. Discuss an approach in an issue before starting a large change; no delivery dates are promised.

## Optional Gemma and local semantic search

Source implemented on `gemma-integration`; build/runtime validation is pending. The published beta does not include this work. See [semantic search notes](docs/SEMANTIC_SEARCH.md).

- [x] Optional EmbeddingGemma 2 text model download during onboarding and later Settings; skipping preserves existing search.
- [x] Download size, disk space/status, cancel/retry, enable/disable and model/vector removal controls.
- [x] Pin the text-only GGUF revision and Windows CPU runtime, SHA-256, license texts and sources.
- [x] Automatic local document/semantic preparation, SQLite chunk vectors and same-field exact/meaning search with fallback. Only selected-root document text; no photos/OCR-image text/audio/video inputs.
- [x] Source handling for integrity verification, interrupted downloads, offline reuse, cancellation, invalidation and fallback.
- [ ] Validate real model download/runtime compatibility, paraphrase retrieval, filters/pagination, interrupted/offline behavior, graceful fallback, reconciliation/root containment, cancellation/worker cleanup and packaged Windows behavior. No builds/tests run for this milestone.
- [x] Automatic semantic refresh after opt-in download, with a persistent bottom activity bar and pause/resume controls (source; runtime validation pending).
- [ ] Measure model startup/ranking costs before considering a persistent inference worker or alternate vector index.

## Automatic app updates

- [ ] Complete the production update path using signed update metadata/artifacts and a trusted release endpoint; configure release signing secrets outside the repository.
- [ ] Add update checks and clear version/release notes, download progress, retry and restart/install confirmation in Settings.
- [ ] Preserve the local library and preferences during upgrades; handle failed/interrupted updates safely.
- [ ] Document the release/signing process and validate real packaged upgrade scenarios on Windows before enabling automatic updates.

Existing update-related source is groundwork; the published beta has automatic updates disabled. Gemma downloads and semantic search are present only in unbuilt branch source.

## Cross-platform releases

- [ ] Add macOS and Linux releases.

## AI search result explanations

- [x] Source snippets and validated matching-passage preview for text/extracted Word, with best-effort PDF page/position location.
- [x] Explicit More related results / Best matches only control with preserved filters and pagination.
- [x] Create English/Urdu/Roman Urdu demo documents and a separate manual testing guide.
- [ ] Run the language/model quality checks manually later; user deferred this task. Validate final snippet/preview layout and PDF navigation in a user-requested build/runtime session.
