# Desktop Fushi Android overlay

The Android app uses the same Rust/wgpu Fushi core as Windows and macOS.
The Android Java side only owns the overlay lifecycle, permission UI, sensors, and a transparent `SurfaceView` that supplies a native rendering surface.

```text
FushiOverlayService  foreground overlay service + sensors
FushiOverlayView     transparent SurfaceView + JNI calls
src/android.rs       ANativeWindow -> raw-window-handle -> WgpuLayer bridge
src/fushi/*          shared physics, expressions, ears, tail, body
```

Build from the repository root:

```bash
./scripts/build.sh android --variant debug
```

For one ABI while testing:

```bash
./scripts/build.sh android --variant debug --abis arm64-v8a
```

Runtime notes:

- Android requires overlay permission before the floating pet can be shown.
- The overlay runs as a foreground service, so Android keeps a small persistent notification while Fushi is floating.
- Linear acceleration is preferred. If unavailable, accelerometer input is high-pass filtered.
- Phone shaking is fed into the shared Rust body through `apply_external_shake`, so Fushi reacts like it is inside a transparent container instead of using an Android-only physics model.
