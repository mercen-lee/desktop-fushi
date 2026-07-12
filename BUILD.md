# Desktop Fushi build layout

Desktop Fushi is now arranged as one Rust codebase with thin platform front-ends.
The shared modules below are used by Windows, macOS, and Android:

```text
src/fushi/          physics, expressions, ears/tail/body renderer
src/gpu_canvas.rs  vector scene tessellation
src/wgpu_layer.rs  wgpu device/surface/offscreen renderer
```

Platform-specific code only supplies native windows/surfaces, input, sensors, and packaging:

```text
src/app.rs         desktop tao front-end for Windows/macOS
src/macos.rs       macOS desktop/window environment helpers
src/win32.rs       Windows layered-window helpers
src/android.rs     Android JNI + ANativeWindow/wgpu bridge
android/           Android overlay service and SurfaceView host
```

## Commands

```bash
./scripts/build.sh                      # host desktop release
./scripts/build.sh desktop              # host desktop release
./scripts/build.sh run                  # run host desktop release
./scripts/build.sh desktop --debug      # host desktop debug
./scripts/build.sh windows --arch all
./scripts/build.sh windows --arch x64
./scripts/build.sh macos --arch arm64
./scripts/build.sh macos --arch universal
./scripts/build.sh android --variant debug
./scripts/build.sh android --variant release
./scripts/build.sh android-rust --abis arm64-v8a,x86_64
```

PowerShell uses the wrapper in the same directory:

```powershell
.\scripts\build.ps1 desktop
.\scripts\build.ps1 windows -Arch x64
.\scripts\build.ps1 macos -Arch arm64
.\scripts\build.ps1 android -Variant debug
.\scripts\build.ps1 android-rust -Abis arm64-v8a,x86_64
.\scripts\build.ps1 run
```

Website builds live in `web/`:

```bash
cd web
npm ci
PUBLIC_DESKTOP_FUSHI_VERSION=0.1.0 npm run build
```

Android uses Gradle variants, so use `--variant debug` or `--variant release`.
Desktop builds use `--debug` / `--release`.

## Outputs

- Host desktop: `target/release/desktop-fushi`
- Windows: `target/<target>/release/Desktop Fushi.exe`
- macOS: `target/<target>/release/Desktop Fushi.app`
- macOS universal: `target/universal-apple-darwin/release/Desktop Fushi.app`
- Android: `android/app/build/outputs/apk/<variant>/app-<variant>.apk`
- Android native libraries: `android/app/src/main/jniLibs/<abi>/libdesktop_fushi.so`

## GitHub automation

GitHub Actions is organized by lifecycle and platform rather than by one large
release workflow:

- `ci-quality.yml` runs Python script compilation plus Rust formatting, checks,
  Clippy, and tests on every pull request and protected-branch push.
- `ci-desktop.yml` smoke-builds Windows x64 and macOS arm64 desktop bundles.
- `ci-android.yml` builds an arm64 debug APK and uploads
  `pocket-fushi-android-arm64-debug`.
- `ci-web.yml` builds the WebAssembly and Astro site without deploying it.
- `cd-release.yml` builds all platform assets from a `vX.Y.Z` tag and publishes
  the GitHub Release. A manual run requires an existing tag and rebuilds that
  exact tag; it cannot publish the branch selected in the Actions UI.
- `cd-web.yml` deploys the website after a release and for relevant `main`
  changes. Main deployments use the latest published release tag for download
  links, so an unreleased Cargo version cannot create broken public URLs.

Runner setup, caches, secrets, and artifact transfer remain in YAML. The
repository-specific behavior lives in `scripts/ci.py` (quality and build
checks) and `scripts/cd.py` (tag validation, packaging, signing, release
verification, GitHub publishing, and Vercel deployment). Android toolchain
setup is shared through `.github/actions/setup-android`.

The CI entry points are also useful locally after the required platform
toolchains are installed:

```bash
python scripts/ci.py quality
python scripts/ci.py desktop --platform macos --arch arm64
python scripts/ci.py android --abis arm64-v8a
python scripts/ci.py web
```

Run `python scripts/cd.py --help` to inspect the release-only packaging,
signing, verification, publishing, and deployment commands.

The release workflow reads `[package].version` from `Cargo.toml`, verifies it
matches `v<version>`, publishes to that tag, and uploads:

- Windows x64 zip
- Windows ARM64 zip
- macOS ARM64 app zip
- macOS universal app zip
- Pocket Fushi Android universal APK

The release jobs run platform builds in parallel and restore Rust caches so
dependency builds do not dominate every release run. Only the publish job has
write permission to repository contents.

The website uses these stable public release asset names:

- `desktop-fushi-v<version>-windows-x64.zip`
- `desktop-fushi-v<version>-windows-arm64.zip`
- `desktop-fushi-v<version>-macos-arm64.zip`
- `desktop-fushi-v<version>-macos-universal.zip`
- `pocket-fushi-v<version>-android-universal.apk`

The Android release APK includes `arm64-v8a`, `armeabi-v7a`, and `x86_64`
native libraries. Gradle produces an unsigned release APK, then the release
workflow aligns and signs it as Pocket Fushi. Configure these repository
secrets before publishing a tag:

- `ANDROID_KEYSTORE_BASE64` — Base64-encoded release keystore
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`

## Android requirements

Install Android SDK + NDK. The build script finds the NDK from one of these places:

```text
ANDROID_NDK_HOME / ANDROID_NDK_ROOT / NDK_HOME
android/local.properties: ndk.dir=...
ANDROID_HOME or ANDROID_SDK_ROOT with an ndk/<version> folder
```

The default ABI set is:

```text
arm64-v8a, armeabi-v7a, x86_64
```

For quick local testing:

```bash
./scripts/build.sh android --variant debug --abis arm64-v8a
```

`FushiOverlayView` is only a transparent `SurfaceView`; `src/android.rs` creates a wgpu surface from Android's native window and runs the same Rust `FushiBody` + `FushiRenderer` path as desktop.

## macOS notes

The macOS app uses wgpu's Metal backend through the same `WgpuLayer` as the other platforms. `./scripts/build.sh macos --arch universal` creates a universal `.app` bundle.
