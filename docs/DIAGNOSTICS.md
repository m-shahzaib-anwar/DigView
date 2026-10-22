# Local logs for reporting issues

Current source enables local diagnostic files by default, independently of the disabled cloud telemetry switches. A new app build is needed to get this behavior in the installed application.

Open **Settings & folders → Local diagnostics → Export logs**, choose a new file name/location, then share that `.jsonl` file with your issue description. **Open logs folder** opens the live files. The login screen also offers **Export local logs**, and the React error screen offers export/reload controls.

On Windows the normal location is `%LOCALAPPDATA%\app.digview.desktop\logs`, resolved through Tauri's app-local-data directory. The app owns `diagnostics.jsonl` and three rotated files. Each rotates at approximately 1 MiB; total retained logs are roughly 4 MiB. Export combines these four files in chronological order into a new file, preserves existing files and omits the live session marker.

Records include app version/platform/architecture, timestamps, app action names and duration, allowlisted error categories, sanitized JavaScript source file/line/column, job counts and cancellation, native panic source location, normal shutdown and the previous session's unclean-exit marker. A forced termination/power loss can produce the same unclean marker as a crash; it does not identify the cause by itself. Native panics are written immediately when the logging lock is available. Frontend error delivery is best effort; an abrupt process termination can lose queued entries.

File contents, user filenames/paths, search queries, recognized text, email addresses, account tokens, passwords, raw exception messages and source URL query strings are excluded. Native code revalidates diagnostic fields before writing. No device/account ID is added. This is a local log, not a full OS crash dump or a recording of everything on screen. Severe OS/WebView crashes may leave only the unclean marker on next launch.

The local writer has no network client. PostHog, Sentry and the older optional metadata ring remain disabled by the four `VITE_*_ENABLED=false` settings documented in [OBSERVABILITY.md](OBSERVABILITY.md). The Windows launcher also forces these cloud-channel flags off while retaining local diagnostics. Nothing is uploaded automatically; export/share is initiated by you.

Logging failures do not interrupt app actions. Settings reports unavailable log storage if writing fails; check disk space and directory permissions, then restart. Exports require a new destination file name and reject the live logs directory. Rotation/export/privacy and unavailable-storage behavior are tested on generated fixtures; actual packaged crash/forced-exit/export UI smoke remains pending.
