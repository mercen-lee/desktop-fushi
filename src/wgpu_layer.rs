use std::sync::mpsc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::gpu_canvas::{GpuScene, GpuVertex};

const SAMPLE_COUNT: u32 = 4;
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

const VECTOR_SHADER: &str = r#"
struct VertexIn {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(in.position, 0.0, 1.0);
    out.color = clamp(in.color, vec4<f32>(0.0), vec4<f32>(1.0));
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let a = clamp(in.color.a, 0.0, 1.0);
    return vec4<f32>(in.color.rgb * a, a);
}
"#;

pub struct LayeredFrame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WgpuSurfaceSize {
    pub width: u32,
    pub height: u32,
}

impl WgpuSurfaceSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
        }
    }
}

pub struct WgpuLayer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    offscreen_pipeline: wgpu::RenderPipeline,
    surface_pipeline: Option<wgpu::RenderPipeline>,
    surface_target: Option<PresentTarget>,
    target: Option<VectorTarget>,
    mesh_buffers: Option<MeshBuffers>,
    readback: Option<ReadbackBuffer>,
}

struct PresentTarget {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    _msaa_texture: wgpu::Texture,
    msaa_view: wgpu::TextureView,
}

struct VectorTarget {
    width: u32,
    height: u32,
    _msaa_texture: wgpu::Texture,
    msaa_view: wgpu::TextureView,
    resolved_texture: wgpu::Texture,
    resolved_view: wgpu::TextureView,
}

struct MeshBuffers {
    vertex_capacity_bytes: u64,
    index_capacity_bytes: u64,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

struct ReadbackBuffer {
    width: u32,
    height: u32,
    padded_bytes_per_row: u32,
    buffer: wgpu::Buffer,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
struct WebDisplayHandleSource;

#[cfg(target_arch = "wasm32")]
impl raw_window_handle::HasDisplayHandle for WebDisplayHandleSource {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(raw_window_handle::DisplayHandle::web())
    }
}

impl WgpuLayer {
    pub async fn new<W>(window: W, size: WgpuSurfaceSize) -> Result<Self, String>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        let instance = create_instance().await;

        #[cfg(target_os = "windows")]
        let surface: Option<wgpu::Surface<'static>> = {
            let _ = window;
            None
        };
        #[cfg(not(target_os = "windows"))]
        let surface: Option<wgpu::Surface<'static>> = Some(
            instance
                .create_surface(window)
                .map_err(|err| format!("failed to create wgpu surface: {err}"))?,
        );

        Self::from_surface(instance, surface, size).await
    }

    #[cfg(all(target_arch = "wasm32", feature = "web"))]
    pub async fn new_for_canvas(
        canvas: web_sys::HtmlCanvasElement,
        size: WgpuSurfaceSize,
    ) -> Result<Self, String> {
        let instance = create_instance().await;
        let surface = Some(
            instance
                .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                .map_err(|err| format!("failed to create web canvas surface: {err}"))?,
        );

        Self::from_surface(instance, surface, size).await
    }

    async fn from_surface(
        instance: wgpu::Instance,
        surface: Option<wgpu::Surface<'static>>,
        size: WgpuSurfaceSize,
    ) -> Result<Self, String> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface.as_ref(),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|err| format!("failed to find a compatible GPU adapter: {err}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Desktop Fushi device"),
                required_features: wgpu::Features::empty(),
                required_limits: required_limits(&adapter),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|err| format!("failed to create wgpu device: {err}"))?;

        let vector_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Desktop Fushi vector shader"),
            source: wgpu::ShaderSource::Wgsl(VECTOR_SHADER.into()),
        });
        let offscreen_pipeline = create_vector_pipeline(&device, &vector_shader, OFFSCREEN_FORMAT);

        let mut surface_pipeline = None;
        let mut surface_target = None;
        if let Some(surface) = surface {
            let width = size.width.max(1);
            let height = size.height.max(1);
            let capabilities = surface.get_capabilities(&adapter);
            let mut config = surface
                .get_default_config(&adapter, width, height)
                .ok_or_else(|| "failed to choose a supported wgpu surface configuration".to_string())?;
            config.format = surface_format(&capabilities, config.format);
            config.alpha_mode = surface_alpha_mode(&capabilities, config.alpha_mode);
            config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
            config.desired_maximum_frame_latency = 2;
            #[cfg(target_os = "android")]
            log::info!(
                "wgpu surface format={:?} alpha_modes={:?} selected_alpha={:?}",
                config.format,
                capabilities.alpha_modes,
                config.alpha_mode
            );
            surface.configure(&device, &config);

            let (msaa_texture, msaa_view) = create_msaa_target(
                &device,
                width,
                height,
                config.format,
                "Desktop Fushi surface MSAA target",
            );
            surface_pipeline = Some(create_vector_pipeline(&device, &vector_shader, config.format));
            surface_target = Some(PresentTarget {
                surface,
                config,
                _msaa_texture: msaa_texture,
                msaa_view,
            });
        }

        Ok(Self {
            device,
            queue,
            offscreen_pipeline,
            surface_pipeline,
            surface_target,
            target: None,
            mesh_buffers: None,
            readback: None,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if let Err(err) = self.ensure_surface_target(width, height) {
            eprintln!("{err}");
        }
        let same = self
            .target
            .as_ref()
            .map(|target| target.width == width && target.height == height)
            .unwrap_or(false);
        if !same {
            self.target = None;
            self.readback = None;
        }
    }

    pub fn render(&mut self, scene: &GpuScene) -> Result<LayeredFrame, String> {
        if self.surface_target.is_some() {
            return self.render_to_surface(scene);
        }
        self.render_to_offscreen(scene)
    }

    fn render_to_surface(&mut self, scene: &GpuScene) -> Result<LayeredFrame, String> {
        let width = scene.width.max(1);
        let height = scene.height.max(1);
        self.ensure_surface_target(width, height)?;
        self.ensure_mesh_buffers(scene.vertices.len(), scene.indices.len());
        self.upload_scene(scene);

        let surface_texture = {
            let target = self
                .surface_target
                .as_ref()
                .ok_or_else(|| "failed to create fushi presentation target".to_string())?;
            match target.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture)
                | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(LayeredFrame {
                        width,
                        height,
                        bgra: Vec::new(),
                    });
                }
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    target.surface.configure(&self.device, &target.config);
                    return Ok(LayeredFrame {
                        width,
                        height,
                        bgra: Vec::new(),
                    });
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err("wgpu surface validation error while acquiring frame".to_string());
                }
            }
        };

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Desktop Fushi surface render encoder"),
            });
        {
            let target = self
                .surface_target
                .as_ref()
                .ok_or_else(|| "failed to create fushi presentation target".to_string())?;
            let pipeline = self
                .surface_pipeline
                .as_ref()
                .ok_or_else(|| "failed to create fushi surface pipeline".to_string())?;
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &target.msaa_view,
                depth_slice: None,
                resolve_target: Some(&surface_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(surface_clear_color()),
                    store: wgpu::StoreOp::Discard,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Desktop Fushi surface vector render pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if !scene.is_empty() {
                if let Some(buffers) = self.mesh_buffers.as_ref() {
                    pass.set_pipeline(pipeline);
                    pass.set_vertex_buffer(0, buffers.vertex_buffer.slice(..));
                    pass.set_index_buffer(buffers.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..scene.indices.len() as u32, 0, 0..1);
                }
            }
        }

        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();
        Ok(LayeredFrame {
            width,
            height,
            bgra: Vec::new(),
        })
    }

    fn render_to_offscreen(&mut self, scene: &GpuScene) -> Result<LayeredFrame, String> {
        let width = scene.width.max(1);
        let height = scene.height.max(1);
        self.ensure_target(width, height);
        self.ensure_readback(width, height);
        self.ensure_mesh_buffers(scene.vertices.len(), scene.indices.len());
        self.upload_scene(scene);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Desktop Fushi render encoder"),
            });
        {
            let target = self
                .target
                .as_ref()
                .ok_or_else(|| "failed to create fushi render target".to_string())?;
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &target.msaa_view,
                depth_slice: None,
                resolve_target: Some(&target.resolved_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Discard,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Desktop Fushi vector render pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if !scene.is_empty() {
                if let Some(buffers) = self.mesh_buffers.as_ref() {
                    pass.set_pipeline(&self.offscreen_pipeline);
                    pass.set_vertex_buffer(0, buffers.vertex_buffer.slice(..));
                    pass.set_index_buffer(buffers.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..scene.indices.len() as u32, 0, 0..1);
                }
            }
        }

        let target = self
            .target
            .as_ref()
            .ok_or_else(|| "failed to create fushi render target".to_string())?;
        let readback = self
            .readback
            .as_ref()
            .ok_or_else(|| "failed to create fushi readback buffer".to_string())?;
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.resolved_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(readback.padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));
        self.read_layered_frame()
    }

    fn read_layered_frame(&self) -> Result<LayeredFrame, String> {
        let readback = self
            .readback
            .as_ref()
            .ok_or_else(|| "failed to create fushi readback buffer".to_string())?;
        let slice = readback.buffer.slice(..);
        let (tx, rx) = mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|err| format!("failed while waiting for GPU readback: {err}"))?;
        rx.recv()
            .map_err(|err| format!("failed to receive GPU readback result: {err}"))?
            .map_err(|err| format!("failed to map GPU readback buffer: {err}"))?;

        let mapped = slice.get_mapped_range();
        let mut bgra = vec![0; readback.width as usize * readback.height as usize * 4];
        let src_stride = readback.padded_bytes_per_row as usize;
        let dst_stride = readback.width as usize * 4;
        if src_stride == dst_stride {
            let frame_len = bgra.len();
            bgra.copy_from_slice(&mapped[..frame_len]);
        } else {
            for y in 0..readback.height as usize {
                let src_row = &mapped[y * src_stride..y * src_stride + dst_stride];
                let dst_row = &mut bgra[y * dst_stride..(y + 1) * dst_stride];
                dst_row.copy_from_slice(src_row);
            }
        }
        drop(mapped);
        readback.buffer.unmap();

        Ok(LayeredFrame {
            width: readback.width,
            height: readback.height,
            bgra,
        })
    }

    fn upload_scene(&mut self, scene: &GpuScene) {
        if let Some(buffers) = self.mesh_buffers.as_ref() {
            if !scene.vertices.is_empty() {
                self.queue.write_buffer(
                    &buffers.vertex_buffer,
                    0,
                    bytemuck::cast_slice(scene.vertices.as_slice()),
                );
            }
            if !scene.indices.is_empty() {
                self.queue.write_buffer(
                    &buffers.index_buffer,
                    0,
                    bytemuck::cast_slice(scene.indices.as_slice()),
                );
            }
        }
    }

    fn ensure_surface_target(&mut self, width: u32, height: u32) -> Result<(), String> {
        let Some(target) = self.surface_target.as_mut() else {
            return Ok(());
        };
        if target.config.width == width && target.config.height == height {
            return Ok(());
        }
        target.config.width = width;
        target.config.height = height;
        target.surface.configure(&self.device, &target.config);
        let (msaa_texture, msaa_view) = create_msaa_target(
            &self.device,
            width,
            height,
            target.config.format,
            "Desktop Fushi surface MSAA target",
        );
        target._msaa_texture = msaa_texture;
        target.msaa_view = msaa_view;
        Ok(())
    }

    fn ensure_target(&mut self, width: u32, height: u32) {
        let needs_new = self
            .target
            .as_ref()
            .map(|target| target.width != width || target.height != height)
            .unwrap_or(true);
        if !needs_new {
            return;
        }

        let msaa_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Desktop Fushi MSAA vector target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let msaa_view = msaa_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let resolved_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Desktop Fushi resolved vector target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let resolved_view = resolved_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.target = Some(VectorTarget {
            width,
            height,
            _msaa_texture: msaa_texture,
            msaa_view,
            resolved_texture,
            resolved_view,
        });
    }

    fn ensure_readback(&mut self, width: u32, height: u32) {
        let padded_bytes_per_row = align_to(width * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let needs_new = self
            .readback
            .as_ref()
            .map(|readback| {
                readback.width != width
                    || readback.height != height
                    || readback.padded_bytes_per_row != padded_bytes_per_row
            })
            .unwrap_or(true);
        if !needs_new {
            return;
        }

        let size = padded_bytes_per_row as u64 * height as u64;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Desktop Fushi readback buffer"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        self.readback = Some(ReadbackBuffer {
            width,
            height,
            padded_bytes_per_row,
            buffer,
        });
    }

    fn ensure_mesh_buffers(&mut self, vertex_count: usize, index_count: usize) {
        let vertex_bytes = (vertex_count * std::mem::size_of::<GpuVertex>()).max(4) as u64;
        let index_bytes = (index_count * std::mem::size_of::<u32>()).max(4) as u64;
        let needs_new = self
            .mesh_buffers
            .as_ref()
            .map(|buffers| {
                buffers.vertex_capacity_bytes < vertex_bytes || buffers.index_capacity_bytes < index_bytes
            })
            .unwrap_or(true);
        if !needs_new {
            return;
        }

        let vertex_capacity_bytes = next_buffer_capacity(vertex_bytes);
        let index_capacity_bytes = next_buffer_capacity(index_bytes);
        let vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Desktop Fushi vector vertex buffer"),
            size: vertex_capacity_bytes,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Desktop Fushi vector index buffer"),
            size: index_capacity_bytes,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.mesh_buffers = Some(MeshBuffers {
            vertex_capacity_bytes,
            index_capacity_bytes,
            vertex_buffer,
            index_buffer,
        });
    }
}

async fn create_instance() -> wgpu::Instance {
    let descriptor = wgpu::InstanceDescriptor {
        backends: preferred_backends(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    };

    #[cfg(target_arch = "wasm32")]
    {
        let mut descriptor = descriptor;
        descriptor.display = Some(Box::new(WebDisplayHandleSource));
        wgpu::util::new_instance_with_webgpu_detection(descriptor).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        wgpu::Instance::new(descriptor)
    }
}

#[cfg(target_arch = "wasm32")]
fn required_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
    wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())
}

#[cfg(not(target_arch = "wasm32"))]
fn required_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
    adapter.limits()
}

fn preferred_backends() -> wgpu::Backends {
    #[cfg(target_arch = "wasm32")]
    {
        return wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
    }

    #[cfg(target_os = "android")]
    {
        return wgpu::Backends::VULKAN | wgpu::Backends::GL;
    }

    #[cfg(target_os = "windows")]
    {
        return wgpu::Backends::DX12;
    }

    #[cfg(target_os = "macos")]
    {
        return wgpu::Backends::METAL;
    }

    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "windows"),
        not(target_os = "macos"),
        not(target_os = "android")
    ))]
    {
        wgpu::Backends::default()
    }
}

fn create_vector_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let vector_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Desktop Fushi vector pipeline layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Desktop Fushi vector pipeline"),
        layout: Some(&vector_pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[vertex_layout()],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: SAMPLE_COUNT,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(premultiplied_blend()),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_msaa_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: &'static str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: SAMPLE_COUNT,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBUTES,
    }
}

fn premultiplied_blend() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

fn surface_clear_color() -> wgpu::Color {
    #[cfg(all(target_arch = "wasm32", feature = "web"))]
    {
        // Keep translucent Fushi colors blending against the page color even when
        // the browser presents the WebGPU canvas as an opaque surface.
        return wgpu::Color {
            r: 0.9647058823529412,
            g: 0.9490196078431372,
            b: 0.9098039215686274,
            a: 1.0,
        };
    }

    #[cfg(not(all(target_arch = "wasm32", feature = "web")))]
    {
        wgpu::Color::TRANSPARENT
    }
}

#[cfg(all(target_arch = "wasm32", feature = "web"))]
fn surface_format(
    capabilities: &wgpu::SurfaceCapabilities,
    fallback: wgpu::TextureFormat,
) -> wgpu::TextureFormat {
    let linear = fallback.remove_srgb_suffix();
    if capabilities.formats.contains(&linear) {
        return linear;
    }
    if let Some(format) = capabilities
        .formats
        .iter()
        .copied()
        .find(|format| !format.is_srgb())
    {
        return format;
    }

    fallback
}

#[cfg(not(all(target_arch = "wasm32", feature = "web")))]
fn surface_format(
    _capabilities: &wgpu::SurfaceCapabilities,
    fallback: wgpu::TextureFormat,
) -> wgpu::TextureFormat {
    #[cfg(target_os = "macos")]
    {
        let linear = fallback.remove_srgb_suffix();
        if _capabilities.formats.contains(&linear) {
            return linear;
        }
        if let Some(format) = _capabilities
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
        {
            return format;
        }
    }

    fallback
}

fn surface_alpha_mode(
    capabilities: &wgpu::SurfaceCapabilities,
    fallback: wgpu::CompositeAlphaMode,
) -> wgpu::CompositeAlphaMode {
    #[cfg(all(target_arch = "wasm32", feature = "web"))]
    {
        if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::Opaque)
        {
            return wgpu::CompositeAlphaMode::Opaque;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            return wgpu::CompositeAlphaMode::PostMultiplied;
        }
        if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::Inherit)
        {
            return wgpu::CompositeAlphaMode::Inherit;
        }
    }

    #[cfg(target_os = "android")]
    {
        if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            return wgpu::CompositeAlphaMode::PreMultiplied;
        }
        if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            return wgpu::CompositeAlphaMode::PostMultiplied;
        }
        if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::Inherit)
        {
            return wgpu::CompositeAlphaMode::Inherit;
        }
    }

    if capabilities
        .alpha_modes
        .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
    {
        wgpu::CompositeAlphaMode::PreMultiplied
    } else {
        fallback
    }
}

fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

fn next_buffer_capacity(bytes: u64) -> u64 {
    bytes.next_power_of_two().max(1024)
}
