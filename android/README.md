# Pocket Fushi Android overlay

The Android app uses the same Rust/wgpu Fushi core as Windows and macOS.
The Android Java side only owns the overlay lifecycle, permission UI, sensors, input, and a transparent `SurfaceView` that supplies a native rendering surface.

```text
FushiOverlayService  foreground overlay service + Choreographer + sensors
FushiOverlayView     transparent SurfaceView + SurfaceHolder lifecycle + JNI input
src/android.rs       JNI mailbox + Rust render thread + ANativeWindow/wgpu bridge
src/fushi/*          shared physics, expressions, ears, tail, body
```

The production rendering path presents wgpu frames directly to the `ANativeWindow`.
It does not rasterize triangles on the CPU, read GPU textures back, or copy frames through an Android `Bitmap`.
The Java/UI thread only publishes input and coalesced `Choreographer` frame requests; a dedicated Rust thread owns physics, tessellation, wgpu, and the native surface.
wgpu selects Vulkan or GLES from the Android backends enabled by the shared renderer.

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
- The `SurfaceHolder` format and overlay window are translucent; Rust uses the shared premultiplied-alpha wgpu pipeline.
- Rendering follows `Choreographer`; frame requests are coalesced so a slow GPU cannot build an unbounded queue. Rust keeps fixed-step 60 Hz physics and renders at 60 fps while active or 30 fps while idle.
- The overlay surface envelope grows in chunks instead of resizing every animation frame, is capped to the current screen after rotation, and follows Fushi by position updates.
- Linear acceleration is preferred. If unavailable, accelerometer input is high-pass filtered.
- Phone shaking is fed into the shared Rust body through `apply_external_shake`, so Fushi reacts like it is inside a transparent container instead of using an Android-only physics model.
- Touch input tracks one active pointer, forwards historical move samples, and supports mouse/stylus hover through the shared Rust hit-testing and drag logic.
