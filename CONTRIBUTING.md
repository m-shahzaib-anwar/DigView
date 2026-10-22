# Contributing to DigView

Bug reports, documentation improvements and pull requests are welcome. Use Issues for reproducible bugs or feature proposals. Please discuss substantial features before implementing them.

## Make a change

1. Fork this repository and create a branch for one focused change.
2. Follow the development setup in [README.md](README.md).
3. Describe the problem, the resulting behavior and how you verified it in your pull request.
4. Add meaningful tests for changed behavior when appropriate. Avoid tests that only repeat UI markup. Include app-only screenshots for visual changes; use generated demo files and remove private information.

The pull-request workflow runs Windows frontend/native validation. The secret-audit workflow checks for exposed credentials. Maintainer review and required checks must complete before merging; maintainers can request changes.

## Project expectations

- Use Tauri 2, Rust, React, TypeScript and SQLite, consistent with the existing architecture.
- Keep scanning, hashing, OCR and document parsing outside the UI thread.
- Index only folders selected by the user. Keep file contents on device.
- Treat filenames, source URLs and document contents as untrusted input.
- Never permanently delete user files. Require confirmation and use OS trash, with identity/containment checks before file actions.
- Preserve existing library/settings during upgrades. Document platform limitations and validation separately from implementation.
- Keep contributions focused and respectful. Explain disagreements with technical evidence; harassment and personal attacks are not acceptable.

## Checks

Run the relevant checks for your change and report the exact outcomes:

```sh
npm test
npm run native:test
npm run build
npm run native:check
```

`npm run native:video` is an explicit Windows codec integration check. `npm run native:integration` moves only generated disposable test duplicates to OS trash; run it deliberately. Do not use personal files as test fixtures.

Do not commit credentials, `.env` files, personal libraries, generated caches or build outputs. Do not include dependency/vendor folders in a PR.

## Security and licensing

Report vulnerabilities through the repository's private **Security → Report a vulnerability** flow; see [SECURITY.md](SECURITY.md). Avoid public issues containing exploit details or private file data.

By submitting a contribution, you agree that your contribution can be distributed under this project's MIT license. Include attribution and required notices for third-party code/assets. See [LICENSE](LICENSE) and [third-party terms](docs/THIRD_PARTY.md).
