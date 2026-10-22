# Signed update preparation

The user-requested unsigned Windows EXE and NSIS installer were built on 2026-10-20; see STATUS.md/releases README for hashes. No signed release, update host or production signing identity was created, and the installed app remains untouched. The source updater is ready for production configuration and stays disabled in this beta.

## Runtime behavior

- Settings → App updates shows the current version, manual Check for updates and an opt-in automatic check. Automatic checks are OFF by default, run on startup/roughly daily while the app runs, and only notify; they never download/install automatically.
- Native code embeds `DIGVIEW_UPDATER_ENDPOINT` and `DIGVIEW_UPDATER_PUBLIC_KEY` from the build process environment. Debug builds, missing/invalid keys or unsafe endpoints disable the updater before requests. These are public settings, not private signing credentials. `.env.local` alone does not configure Rust.
- Uses pinned official `tauri-plugin-updater` 2.14.0. Manifest endpoint must be public HTTPS without credentials/query/fragment. Artifact URLs and at most five redirects must remain public HTTPS; CDN query strings are allowed and are never passed into logs. Native code accepts no renderer-supplied URL/key.
- Requires a newer version, `requireSignedVersion=true`, `allowDowngrades=false`, valid artifact signature and an authenticated version matching the announcement. Shared native policy verifies the signed bytes/version again before install. Public-key decoding fails closed. No TLS/signature bypass or custom download executor exists.
- Pending offers expire after one hour. The explicit confirmation binds installation to the displayed version. A concurrent update request or running library task is refused. The UI also blocks installation during OCR work. Downloads have a 120-second request timeout; failures require checking again.
- After signature verification, the normal engine job slot is reserved. `VACUUM INTO` creates a consistent SQLite snapshot in the app's own `update-backups/`; backup failure aborts installation. The updater launches the installer, and Windows exits the app. Selected roots, favourites, saved searches, OCR index and scan preferences remain in the same SQLite data path. Theme/update-check WebView storage, background.json and the account vault remain at their existing paths; they are not included in the database snapshot and are never deleted by this flow.
- Stable product name, app identifier, binary name and per-user installation scope are retained. `tauri.updates.conf.json` is an opt-in release overlay enabling signed updater artifacts/currentUser NSIS. Restore/install behavior still needs real packaged upgrade testing; source tests do not prove an installer upgrade.

## Release prerequisites and future commands

Before the user-requested final build:

1. Choose a production HTTPS manifest/artifact host. No release was uploaded or published during implementation.
2. Generate and securely retain a production Tauri signing pair outside the repository. Keep its private key/password in local secure build inputs or CI secrets, never in chat, frontend code, Git, or the public manifest. The public key must remain stable for the installed update channel.
3. Configure public `DIGVIEW_UPDATER_ENDPOINT` and `DIGVIEW_UPDATER_PUBLIC_KEY` in the shell that runs the release build. Configure `TAURI_SIGNING_PRIVATE_KEY` and its password using the official CLI's secure build setup.
4. Obtain/configure a Windows Authenticode signing certificate and timestamp setup for publisher trust. This is separate from Tauri's updater key pair; neither a certificate nor a production key was invented/generated here.
5. Bump the app version coherently, then build/bundle with `src-tauri/tauri.updates.conf.json` only after the user asks. The existing sequential frontend/native embedding workflow still applies; do not replace dist while Rust embeds it. `tauri build` binds the artifact signature to the app version; manual signing must pass `--app-version`. The installed CLI's help confirms this option.
6. Publish the newer version's platform manifest, signed installer and matching .sig, then test actual signature rejection, offline/retry, cancel/confirmation, same-directory update, database/settings/session preservation and backup recovery on a disposable installation before public distribution.

Official references: [Tauri updater](https://v2.tauri.app/plugin/updater/), [updater API/source](https://docs.rs/tauri-plugin-updater/2.14.0/tauri_plugin_updater/). Runtime defaults and release overlay retain signed-version enforcement.

## Evidence

Native tests use a disposable text artifact signed with the installed Tauri CLI using `--app-version 0.2.0`. Only its public key, text artifact and signature are stored as source fixtures; the throwaway test key is in ignored .tools and is not a production identity. Tests prove valid acceptance and rejection of changed bytes, unsigned input, mismatched announcement and forged trusted-comment version. They also check endpoint policy and database snapshot/migration preservation. Frontend tests exercise disabled checks, explicit matching-version confirmation, busy refusal and propagated native failures. No test contacts a live updater service or runs an installer; desktop-adapter compilation and end-to-end upgrade remain pending by user instruction.
