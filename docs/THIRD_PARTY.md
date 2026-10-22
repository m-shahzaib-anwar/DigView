# Third-party components

DigView's own code is licensed under the root [MIT License](../LICENSE). Dependencies retain their individual terms. The notice generator includes the application MIT text alongside third-party notices, so future installer/direct-EXE notice resources carry both. Existing packages need rebuilding to receive it.

App artwork is original vector artwork in app-icon.svg and CSS; browser preview landscape illustrations are generated from local SVG strings.

Dependencies and exact versions are recorded in package-lock.json and src-tauri/Cargo.lock. Distribution must preserve each dependency's notices. The OCR preparation script includes the Tesseract.js, Tesseract core and upstream language-model licenses alongside the worker assets. The English npm package declares MIT in its metadata; its upstream [language-data repository](https://github.com/naptha/tessdata) publishes Apache-2.0 terms, included from its LICENSE file.

`python scripts/notices.py` collects production npm notices and all locked Cargo package notices from downloaded archives into `src-tauri/THIRD_PARTY_NOTICES.txt`, included as an installer resource. The Cargo listing deliberately includes other OS/build dependencies. The 2026-10-21 review resolved all **48 missing packaged-text entries** through version-pinned upstream texts/declarations, recorded in `notices/upstream-review.json`. Every supplement has a source URL and SHA-256; the collector checks package version, archive/integrity identity, declared license and evidence checksum before using it. Changed versions receive no inherited approval. Generation is offline after dependencies are fetched; it never runs package scripts. `docs/notice-review.txt` now reports **zero unresolved missing texts**. This measures notice coverage, not completed commercial legal clearance.

Some upstream repositories provide only a license declaration, not full license boilerplate. Those entries explicitly preserve the exact-version package declaration/author metadata and canonical SPDX terms; no copyright holder/year was invented. The canonical MIT text's copyright placeholder is a template, not a claim about a package. objc2's original licensing explanation, including its Apple SDK caveat, is retained; macOS SDK distribution rights remain outside this Windows/source-notice review. `@tesseract.js-data/eng` retains both its npm MIT declaration and upstream language data's Apache-2.0 terms.

For multi-license expressions the supplemental review selects MIT where offered, including `r-efi` (no LGPL selection). All other original archive notices remain intact. The five MPL-only crates (`cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext`, `selectors`) have exact-version source archive URLs and Cargo checksums in `notices/MPL-SOURCES.txt`, appended to the distributed notice resource. Their source is unmodified; app-specific license restrictions must not restrict recipients' rights to these third-party sources. See [Mozilla's MPL FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/) and the included full terms.

The optional Node canvas dependency is conservatively included in the dependency inventory; the Tauri package configuration does not ship `node_modules`/its native `.node` binary. This review does not approve an alternate distribution that bundles those binaries or all platform SDKs. Existing packaged EXEs/installers are unchanged and need a new build to carry the refreshed notice resource. See `PUBLIC-READINESS.md` for the audit scope and remaining public-release work.

- Tauri: https://github.com/tauri-apps/tauri
- React: https://github.com/facebook/react
- TanStack Virtual: https://github.com/TanStack/virtual
- Lucide icons: https://github.com/lucide-icons/lucide
- Tesseract.js: https://github.com/naptha/tesseract.js
- Tesseract.js core: https://github.com/naptha/tesseract.js-core
- Bundled English data: https://github.com/naptha/tessdata
- Rusqlite: https://github.com/rusqlite/rusqlite
- Image: https://github.com/image-rs/image
- Trash: https://github.com/Byron/trash-rs

No proprietary reference-app assets or branding are bundled.
