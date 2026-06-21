pub mod canvas;
pub mod desktop;
pub mod fushi;
pub mod gpu_canvas;
pub mod math;
pub mod wgpu_layer;

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub mod app;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub mod settings;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod single_instance;
#[cfg(target_os = "windows")]
pub mod win32;

#[cfg(target_os = "android")]
pub mod android;
#[cfg(all(target_arch = "wasm32", feature = "web"))]
pub mod web;
