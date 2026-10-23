# Project instructions

Build this app autonomously from the documented DigView feature scope. Read docs/STATUS.md and docs/FEATURES.md before continuing work. Keep docs current after each milestone so another session can resume without chat history.

Use Tauri 2, Rust, React, TypeScript, SQLite. All scanning, hashing and OCR must run outside the UI thread. Keep file contents on device. Never permanently delete user files: explicit confirmation and the OS trash only. Never scan arbitrary folders before the user selects them. Treat source URLs and filenames as untrusted input.

Do not claim feature parity, faster performance, or cross-platform testing without evidence. Track implementation separately from validation. Meaningful tests: search interpretation, duplicate identity, scan reconciliation, root containment, cancellation, trash revalidation. Avoid tests that merely mirror UI markup.

No subagents required. Update docs/STATUS.md with commands, test outcomes, blockers and next actions before ending a work session.

User preference (2026-10-22): do not automatically run compile checks, frontend/native builds or installer packaging after changes. Make source/documentation changes only; the user will build at the end. Run compilation/build/packaging only when the user explicitly requests it. Do not run tests unless explicitly requested.

User preference update (2026-10-23): after code updates, run only relevant necessary tests. Never build a desktop application EXE, frontend/native application bundle or installer unless the user explicitly asks. Native test harness compilation is allowed as part of running necessary tests; do not run standalone compile checks or app build commands.
