# Desktop Fushi

Desktop Fushi is a transparent desktop and Android overlay pet rendered with Rust and wgpu.

Windows, macOS, and Android share the same Fushi physics, expressions, vector renderer, GPU tessellation, and wgpu presentation path.

## Project Layout

```text
src/fushi/          Shared physics, expressions, body, tail, and vector renderer
src/gpu_canvas.rs  VectorCanvas to GPU mesh scene
src/wgpu_layer.rs  Shared wgpu device, surface, and offscreen renderer
src/app.rs         Windows/macOS desktop window and event loop
src/android.rs     Android JNI bridge for SurfaceView and wgpu
android/           Android overlay UI, foreground service, sensors, and SurfaceView host
```

## Build and Run

Use the wrapper in `scripts/`. Desktop builds are release builds by default; Android builds a debug APK by default.

```powershell
.\scripts\build.ps1 desktop
.\scripts\build.ps1 windows -Arch arm64
.\scripts\build.ps1 windows -Arch all
.\scripts\build.ps1 android -Variant debug -Abis arm64-v8a
.\scripts\build.ps1 run
```

On macOS or Linux:

```bash
./scripts/build.sh desktop
./scripts/build.sh macos --arch arm64
./scripts/build.sh android --variant debug
./scripts/build.sh run
```

## Commit Convention

Use concise Conventional Commit-style messages:

```text
<type>: <Imperative English summary>
```

Examples:

```text
feat: Add Android overlay service
fix: Restore transparent window hit testing
docs: Update build instructions
```

Use only these project types:

| Type | Use for |
| --- | --- |
| `feat` | New user-facing app, renderer, desktop, Android, or web behavior |
| `fix` | Bug fixes and behavior corrections |
| `refactor` | Internal structure changes without intended behavior changes |
| `style` | Formatting, CSS, visual polish, icons, and layout changes |
| `docs` | README, BUILD, comments, and other documentation |
| `test` | Test coverage, fixtures, and verification code |
| `chore` | Build scripts, CI, dependency, release, and repository maintenance |
| `security` | Permissions, privacy, signing material, and secret-handling changes |
| `perf` | Rendering, startup, memory, binary size, and build performance |

## Notes

- See `BUILD.md` for output paths, Android NDK setup, and macOS bundle notes.
- Windows release artifacts are copied to `Desktop Fushi.exe`.
- Android requires overlay permission and runs as a foreground service while the pet is floating.
- The Fushi character copyright belongs to TWIN ENGINE.
