# Contributor TODO

Planned work below is not available in the current beta. Discuss an approach in an issue before starting a large change; no delivery dates are promised.

## Optional Gemma and local semantic search

- [ ] Add an optional Gemma selection during onboarding; selecting it downloads the model. Skipping it must leave filename and existing document-text search usable.
- [ ] Let users enable, download, cancel/retry and remove the model later from Settings, with download size, progress, disk requirements and clear status.
- [ ] Choose and document the model/version, supported hardware, model license and download source before implementation. Do not assume a particular model size or bundle weights under the app's MIT license.
- [ ] Run inference/indexing on background workers, keep file contents on device, and process only user-selected folders with the relevant opt-in.
- [ ] Validate model integrity, interrupted downloads, offline use and graceful fallback when the model is unavailable.

## Automatic app updates

- [ ] Complete the production update path using signed update metadata/artifacts and a trusted release endpoint; configure release signing secrets outside the repository.
- [ ] Add update checks and clear version/release notes, download progress, retry and restart/install confirmation in Settings.
- [ ] Preserve the local library and preferences during upgrades; handle failed/interrupted updates safely.
- [ ] Document the release/signing process and validate real packaged upgrade scenarios on Windows before enabling automatic updates.

Existing update-related source is groundwork; the published beta has automatic updates disabled. Gemma downloads and semantic search are also not included yet.
