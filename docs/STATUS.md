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
