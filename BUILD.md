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

## GitHub release automation

`.github/workflows/release.yml` builds release artifacts on pushes to
`vX.Y.Z` tags and on manual `workflow_dispatch` runs. The workflow reads
`[package].version` from `Cargo.toml`, verifies that release tag pushes match
`v<version>`, publishes to that release tag, and uploads:

- Windows x64 zip
- Windows ARM64 zip
- macOS ARM64 app zip
- macOS universal app zip

The CD jobs run platform builds in parallel and restore Rust caches so
dependency builds do not dominate every release run.

After release assets are uploaded, `deploy_web` builds the Astro site with
`PUBLIC_DESKTOP_FUSHI_VERSION=<Cargo package version>` and deploys the prebuilt
output to Vercel. Vercel production deploys should come from that job rather
than Git push auto-deploys, so release download links always point at uploaded
`v<version>` assets.

The website uses these stable public release asset names:

- `desktop-fushi-v<version>-windows-x64.zip`
- `desktop-fushi-v<version>-windows-arm64.zip`
- `desktop-fushi-v<version>-macos-arm64.zip`
- `desktop-fushi-v<version>-macos-universal.zip`

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
