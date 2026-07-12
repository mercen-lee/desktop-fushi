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
- Linear acceleration and gravity sensors are used together when both are available. If either is unavailable, the raw accelerometer is used and Rust separates gravity with a time-based filter.
- Sensor samples include the current display rotation and are merged in a bounded Rust mailbox. Ordinary handling, slow tilting, single bumps, and hand tremor never create shake impulses; only a confirmed strong back-and-forth shake starts container bouncing.
- Filtered gravity is independent of shake detection and continuously follows the phone's current orientation during ordinary flight and container bouncing. A nearly flat phone adds almost no screen-plane gravity, while a steeper tilt pulls Fushi toward the physical low side. Detector-integrated velocity change scales each inertial kick, so faster shakes transfer more speed. While the deliberate-shake gate is open, high normal restitution and tangent retention produce energetic ice-like rebounds. As soon as that gate closes, gravity carries any remaining flight only until the next real wall contact, where Fushi immediately reattaches instead of micro-bouncing.
- Motion sensors are optional and their selected mode (`direct pair`, `raw accelerometer`, or `none`) is sent explicitly to Rust. Devices without a usable sensor disable Android container motion and keep the original screen-down behavior without additional permission.
- Touch input tracks one active pointer, forwards historical move samples, and supports mouse/stylus hover through the shared Rust hit-testing and drag logic.
