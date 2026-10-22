# Windows EXE aur installer banane ka process

Current source uses the DigView executable and application identifier. Quit the previous app before the next install/first launch; Windows migration source preserves the old library/profile and imports them once. Identity migration and installed upgrade remain unverified; see [installer notes](docs/INSTALLER.md).

Yeh guide **Windows x64 local unsigned beta** ke liye hai. Root folder mein `Build-Windows.cmd` double-click karne se tests, frontend, optimized native EXE aur NSIS installer sequence mein bante hain. App/installer automatically open ya install nahi hota.

## Double-click se build

1. Project isi complete folder mein rakhein. Doosra build ya `npm run dev` band rakhein; build ke dauran frontend files generate na karein.
2. **Build-Windows.cmd** double-click karein. Admin mode ki zaroorat nahi.
3. Console khuli rehne dein. Tests fail hon to build ruk jayega. Aakhir mein **BUILD SUCCESS** ka wait karein; Enter dabane se console close hoti hai.
4. `releases/LATEST-BUILD.txt` mein latest successful output folder ka path milega. Har run ka alag folder: `releases/builds/<UTC-time>-<unique-id>/`.

Successful folder mein:

- `digview.exe` — direct app executable.
- `DigView-<version>-windows-x64-setup.exe` — installer.
- `THIRD_PARTY_NOTICES.txt` — direct EXE ke saath rakhein.
- `checksums.json` — copied files ke verified SHA-256 hashes aur sizes.
- `build-info.json`, `BUILD-SUCCESS.txt` aur `build.log` — build details/result.

Pichle releases overwrite nahi hote. Failed run ka log uske apne folder mein rehta hai; missing-prerequisite error console mein dikhta hai, output folder banne se pehle. Terminal se bhi same launcher: `npm run build:windows`. Sirf steps dekhne ke liye: `node scripts/build-windows.mjs --plan`; prerequisite paths/version check ke liye: `node scripts/build-windows.mjs --check` (dono build nahi karte).

Build time machine/cache par depend karta hai. Pichla verified native release build 22m50s laga tha; yeh next build ka time estimate nahi hai. Build cache rakhne se incremental compilation possible hai.

## Is machine par required setup

- Node.js **22.13+ (22.x) ya 24+**, Python **3.11+** (`python` ya `py -3`), Visual Studio C++ Build Tools with MSVC x64 aur Windows SDK. PDF.js renderer ki Node requirement ki wajah se minimum update hua hai.
- Installed JavaScript dependencies in `node_modules`. Missing hon to project root PowerShell mein `npm ci` chalayein; network chahiye aur OCR/PDF assets bhi prepare honge.
- Existing working Rust toolchain: `.tools/toolchain/bin/cargo.exe` aur `rustc.exe`; cached crates `.tools/cargo/registry`. Wrappers isi local toolchain ko use karte hain. Rust minimum version `src-tauri/Cargo.toml` mein hai; current setup 1.99.1 se build hua tha.
- Cached NSIS: `src-tauri/target/.tauri/NSIS/makensis.exe`. Local bundling config isi cache ko use karta hai.

**Current project copy ke liye ready hai.** Launcher dependencies/downloads/installers automatically install nahi karta. `.tools` ya NSIS cache delete kar diya ho to pehle working toolchain/crate/NSIS caches restore karein. Fresh machine setup is offline launcher ka hissa nahi hai. `node_modules` Git mein nahi hota; `.tools` aur target cache bhi portable source checkout ka hissa nahi hain.

Login/account configuration required nahi hai. App local library kholti hai; private/server secrets Vite variables mein na rakhein. Launcher `.env.local` edit nahi karta.

## Installer aur pehli launch

NSIS installer approved green DigView icon/header/sidebar ke saath current Windows user ke liye install karta hai. Standard directory, shortcuts, upgrade/uninstall aur final **Run DigView** option retained hain. Pehli launch par flat full-window setup: Welcome → Folders → Search → Preferences. Summary/Review step nahi hai. Preferences par **Start using DigView** confirmation se pehle naye chosen folders scan nahi hote. Folders hon to direct library; folders na hon to optional Add folders/Open my library screen. **Set up later** existing library/settings rakhta hai aur isi routing ko follow karta hai. Purane build ki automatically skipped setup state ab ek dafa corrected wizard kholegi; completion/Set up later ke baad restarts aur retained-data reinstall seedha library kholenge. Wizard dobara kholne ka option nahi hai; preferences normal Settings mein edit hoti hain. Details: [INSTALLER.md](docs/INSTALLER.md).

Installer bitmaps source mein included hain. Logo change ke baad Windows par regenerate karein: `node scripts/prepare-installer.mjs`. Yeh existing native icon se 24-bit BMP banata hai; PowerShell execution policy change nahi karta.

Folders step mein ab Documents, Pictures, Videos, Music, Desktop aur Downloads OS ke actual configured paths se suggest hote hain. Naye options unchecked hain; select aur final confirmation ke baad index honge. Missing/overlapping existing locations disabled hain; Add a folder bhi available hai.

## Build mein kya hota hai

1. Windows/Node/Python/toolchain/cache aur teen manifests ki matching app version check.
2. Single-run lock: doosra launcher refuse hota hai jab pehla runner ya uska tracked worker chal raha ho.
3. Frontend/build-runner tests aur native core fixture tests.
4. Dependency notices refresh, bundled OCR/PDF assets prepare, TypeScript check, Vite build.
5. Frozen `dist` ke saath optimized native EXE compile. Native build hook se frontend dobara generate nahi hota.
6. Usi EXE se NSIS installer package. `--no-binary-patching` se packaging EXE ko modify nahi karti.
7. Dist/EXE consistency, fresh installer timestamp aur copied file hashes verify. Phir successful release record/latest path save.

Child processes mein cloud telemetry OFF, live updater config unset aur Cargo offline rehta hai; `.env` files edit nahi hotin. Device par bounded local diagnostic logs enabled hain; Settings se export/share karein ([guide](docs/DIAGNOSTICS.md)). Yeh local beta profile hai. **Production signing/secure update publication automatic nahi hai**; certificate, keys aur hosting ke liye [UPDATES.md](docs/UPDATES.md) dekhein. Dependency notices generate hona commercial clearance ki guarantee nahi hai.

## Manually PowerShell se

Project root mein PowerShell kholein. Har command ka successful exit check karke agla chalayein; error ke baad packaging ya copying continue na karein.

```powershell
$env:CARGO_NET_OFFLINE = 'true'
$env:CARGO_TARGET_DIR = $null
$env:DIGVIEW_UPDATER_ENDPOINT = $null
$env:DIGVIEW_UPDATER_PUBLIC_KEY = $null
$env:TAURI_SIGNING_PRIVATE_KEY = $null
$env:TAURI_SIGNING_PRIVATE_KEY_PATH = $null
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $null
$env:VITE_OBSERVABILITY_ENABLED = 'false'
$env:VITE_POSTHOG_ENABLED = 'false'
$env:VITE_SENTRY_ENABLED = 'false'
$env:VITE_DIAGNOSTICS_ENABLED = 'false'

npm test
node scripts/native.mjs test --manifest-path native-core/Cargo.toml --lib --locked
python scripts/notices.py
npm run build

# npm run build ke baad dist ko dobara generate na karein.
$env:CARGO_TARGET_DIR = Join-Path (Get-Location).Path 'src-tauri/target'
npm run tauri -- build --no-bundle --no-sign --config scripts/windows-build.config.json
npm run tauri -- bundle --bundles nsis --no-sign --config scripts/windows-bundle.config.json --no-binary-patching
```

`python` unavailable aur `py` available ho to notices command `py -3 scripts/notices.py` use karein. `npm run build` ka prebuild OCR/PDF assets prepare karta hai. Native fixture tests se pehle custom `CARGO_TARGET_DIR` unset rakhein.

Successful manual build ke raw outputs:

```text
src-tauri/target/release/digview.exe
src-tauri/target/release/bundle/nsis/DigView_<version>_x64-setup.exe
src-tauri/THIRD_PARTY_NOTICES.txt
```

Manual commands timestamped delivery folder, copy verification aur latest pointer automatically create nahi karte. Unke liye double-click launcher use karein. Manual hash check example:

```powershell
Get-FileHash -Algorithm SHA256 -LiteralPath 'src-tauri/target/release/digview.exe'
Get-FileHash -Algorithm SHA256 -LiteralPath 'src-tauri/target/release/bundle/nsis/DigView_0.1.0_x64-setup.exe'
```

Future version ke liye `package.json`, `src-tauri/Cargo.toml` aur `src-tauri/tauri.conf.json` mein version same set karein. Launcher version automatically bump nahi karta. Existing install ko update karte waqt app identifier/product identity change na karein.

## Failure aur install

- **Windows real-video smoke / HRESULT 0x8004B200:** actual Windows Shell codec smoke ab default unit gate se separate hai. Default build deterministic core tests chalata hai. Real host check manually: `npm run native:video`; yeh sirf existing generated MP4 fixture use karta hai, aur real frame/color/size assertions fail hon to command fail hi hoti hai. Video extraction fail ho to production app video icon dikhati hai. Is separate check ka pass hona installation/codec availability ki evidence hai; build pass hona video runtime proof nahi.

- **Tests/compilation failed:** `build.log` ki last error fix karein, launcher dobara chalayein. Purana installer latest result samajh kar use na karein.
- **Cargo offline dependency missing:** working cache restore karein. Agar dependency change ki hai to consciously online fetch karein: `$env:CARGO_NET_OFFLINE = 'false'`, phir `node scripts/native.mjs fetch --manifest-path src-tauri/Cargo.toml --locked`. Agla launcher phir offline use karega. Network/proxy issues isse alag hain.
- **Packaging os error 32 / sharing violation:** launcher 3 seconds baad sirf ek retry karta hai. Persistent error par target EXE/installer ki preview ya use band karke retry karein. Launcher user apps ko close nahi karta.
- **Another build/worker running:** us build ko finish/cancel hone dein. Running build ke dauran `.tools/windows-build.lock` manually remove na karein. Dead runner aur dead tracked worker ki stale lock agla launcher recover karta hai. Manual CLI builds launcher lock nahi lete, isliye unhein saath na chalayein.
- **Cancel:** console mein Ctrl+C; launcher apne directly spawned build process tree ko stop karne ki koshish karta hai. Console zabardasti close karne se workers reh sakte hain; fresh build se pehle confirm karein ke previous build stopped hai.
- **Install:** existing app ko tray se **Quit** karein, phir successful installer manually open karein. Same version 0.1.0 ke liye **Add/Reinstall** aur detected existing folder rakhein. Actual in-place upgrade/library/session preservation smoke abhi pending hai.

Validation (2026-10-21): launcher ka full EXE/NSIS build successful hai: `releases/builds/20261021-182320Z-c4b5ac94/`. 69 frontend, 69 native checks passed (3 intentional ignored), TypeScript/Vite/notices/optimized EXE/package aur copy/hash checks passed. Installer execute nahi hua; actual installation/uninstallation/upgrade/native onboarding smoke pending hain. Build/fixture checks app runtime ya production signing validation nahi hain.

Latest correction (2026-10-22): `releases/builds/20261021-185337Z-setup-fix/` mein old-library setup skip fix aur one-time completion behavior hai. TypeScript/Vite/native release/NSIS/hash checks passed; is correction ke fixture tests rerun nahi hue. Actual installation/native restart smoke abhi pending hai.
