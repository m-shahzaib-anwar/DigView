# Cloud analytics/reporting (prepared, OFF)

Current source also has a separate, enabled-by-default **local file logger**, requested for manual issue reporting. See [DIAGNOSTICS.md](DIAGNOSTICS.md) for Settings/login export controls, rotation and privacy. The switches below apply to `src/observability.ts`; they do not disable this separate native file logger.

Updated 2026-10-20. User requested preparing analytics/logging while keeping collection disabled during active development. No PostHog/Sentry account, project or dashboard is created by this code. No credentials have been supplied and no live report was sent.

## Current switches

All four flags below are explicitly false in ignored `.env.local` and `.env.example`. Missing values also mean false. `import.meta.env.PROD` is additionally required, so the Vite development server cannot collect even when flags are accidentally true.

```dotenv
VITE_OBSERVABILITY_ENABLED=false
VITE_POSTHOG_ENABLED=false
VITE_SENTRY_ENABLED=false
VITE_DIAGNOSTICS_ENABLED=false
VITE_POSTHOG_TOKEN=
VITE_POSTHOG_HOST=https://us.i.posthog.com
VITE_SENTRY_DSN=
```

OFF means `src/observability.ts` installs no collection listeners/timers, IDs, storage reads/writes, queues or network requests. Its source hooks are no-ops. Disabling does not erase data collected by a previously enabled version; internal `observability.clear()` removes this feature's two localStorage keys if needed. Local file diagnostics are independent; installed artifacts still need a new build for that addition.

## Prepared channels

- **PostHog:** explicit manual Capture API events, using `/batch/`; no SDK/autocapture/remote scripts/replay or DOM collection. App start, workspace readiness, screen enum, selected native-command outcomes/durations, observed scan/duplicate/watch completion. Once enabled, a random installation ID associates anonymous events with a device; there is no account identity hook. OS/app version are populated from bootstrap. No hardware fingerprint, email/name or browser fingerprint. All events set `$process_person_profile=false`. Legacy queued identity fields are discarded on load; no person profile is created.
- **Sentry:** minimal sanitized error envelopes, not the full SDK. Frontend unhandled error/rejection and caught file-command failures report fixed event/error codes and safe operation metadata. No original exception message/stack/request/context/breadcrumbs or user profile is forwarded. Error reports group by event/action/code. No replay, tracing or performance profiling.
- **Optional metadata ring:** bounded structured metadata ring in WebView localStorage. Internal `observability.localRecords()` provides sanitized event/time/properties records without device IDs. This older ring stays OFF; the new log export UI uses the separate native logger in DIAGNOSTICS.md.

All channels need master switch + production mode; each also needs its channel flag. PostHog additionally needs a public `phc_` project token and exact US/EU ingestion origin. Sentry needs a public hosted DSN with numeric project, hex public key and `o<number>.ingest[.us|.de].sentry.io` host. Personal/admin/secret keys and arbitrary/self-hosted endpoints are unsupported. Credentials are not written into queued diagnostic records. CSP allows the fixed PostHog origins and hosted Sentry ingestion domains; runtime endpoint validation further restricts sends. No new Rust dependency, native HTTP permission or remote script permission.

## Privacy and queue behavior

Properties are allowlisted numeric counts/durations, booleans, fixed view/action/job/error/platform enums and numeric dotted app version. Filenames, paths, contents, OCR, thumbnails, source URLs, search text, emails, OTP, passwords and bearer tokens are dropped. Global errors deliberately omit all raw error details. Reports use HTTPS, no cookies/referrer, redirects rejected and five-second timeout. As with any hosted ingestion, the receiving network sees the connecting IP; no IP property or geolocation enrichment is intentionally added (PostHog `$geoip_disable=true`).

Enabled capture persists up to 100 metadata records for seven days, oldest dropped at capacity. Fifteen-second flush and online reconnect retry delivery in batches (PostHog 20, Sentry 5), with backoff capped at five minutes. Each sink records success independently, so a failed service does not block the other or replay already acknowledged delivery in the current queue. Requests in flight at process death can still be redelivered after restart; PostHog UUID/Sentry event ID is retained. Delivery is best effort, not guaranteed exactly once. Repeated identical error categories are limited to one per 30 seconds per window. Storage/network failures do not throw into normal file operations. Nothing waits for network delivery during shutdown.

## Timing boundaries and remaining validation

`app_started.duration_ms` is renderer document age when diagnostics is installed; it is **not native process startup time**. `workspace_ready.duration_ms` is App mount-to-initial-bootstrap-settlement time, including failures. Native commands use renderer monotonic request/response duration. `job_finished.duration_ms` comes from the existing native Job; only running-to-idle transitions observed by polling are recorded, so very short/unobserved jobs can be missed. `window_unload` is best-effort pagehide observation, not an OS shutdown-duration measurement. Hiding to tray is not window unload.

Full Rust panic/native fatal-crash dumps, process startup/shutdown timings, symbolication and automatic native logs require separate native integration/validation later. No claim of crash-dump coverage or production delivery. No Rust source was changed or compiled for this milestone.

When the user asks to activate: obtain PostHog public project token + US/EU ingestion host, and Sentry public DSN if desired; confirm intended data/identity and user privacy controls; set only the desired channel flags and master true; perform the final requested build, native WebView CSP/CORS/network checks and inspect actual dashboards with synthetic non-file data. Test default-OFF network/storage absence in the packaged app, online/offline/restart/reconnect, sign-out identity, capacity/expiry and rate limits before public release. Do not activate during current development or request secret keys.

## Validation and references

See STATUS.md for exact test outcomes. Cloud-channel tests use in-memory storage and mocked HTTP; they do not contact a service. Live cloud ingestion remains unverified/disabled. The separate local file logger/export has source and fixture validation; actual packaged crash/export UI smoke remains pending.

- [PostHog Capture API](https://posthog.com/docs/api/capture)
- [PostHog API overview](https://posthog.com/docs/api)
- [Sentry envelope format](https://develop.sentry.dev/sdk/envelopes/)
- [Sentry transport implementation](https://github.com/getsentry/sentry-go/blob/master/transport.go)
