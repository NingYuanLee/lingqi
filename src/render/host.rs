//! 自研渲染后端的宿主：自己建窗口、设备与交换链，跑 egui，用自写渲染器出帧。
//!
//! 与 egui-wgpu 的差别只在一处：**谁持有交换链**。
//! egui-wgpu 由 eframe 建窗口与交换链；这里由 [`Host`] 建，
//! 于是渲染器、合成树、呈现节奏都在自己手里。

use crate::render::renderer::{Renderer, ScreenDescriptor};
use crate::render::surface::{configure, create_surface, CompositionTarget};
use windows::Win32::Foundation::HWND;

/// 描述符表上限。DX12 会按这个上限预建整张描述符表：默认 100 万项约 61MB，
/// 65536 项约 4MB，对灵栖的几张纹理仍然有上千倍余量。
const MAX_BINDINGS: u32 = 65_536;

/// 设备上限：小块显存分配 + 收紧描述符表。
///
/// 与 egui 路径（见 `graphics.rs`）保持一致，否则同一张显卡上两个后端的
/// 内存占用会差出几百 MB——那不是渲染器的问题，是 DX12 分配器的默认块大小。
fn small_limits() -> wgpu::Limits {
    wgpu::Limits {
        max_texture_dimension_2d: 8192,
        max_non_sampler_bindings: MAX_BINDINGS,
        ..wgpu::Limits::downlevel_webgl2_defaults()
    }
}

/// 自研后端的宿主。
pub struct Host {
    /// 岛体窗口。
    pub hwnd: HWND,
    /// 渲染设备。
    pub device: wgpu::Device,
    /// 提交队列。
    pub queue: wgpu::Queue,
    /// DComp 交换链。
    pub surface: wgpu::Surface<'static>,
    /// 合成目标。
    pub target: CompositionTarget,
    /// 自写渲染器。
    pub renderer: Renderer,
    /// egui 上下文（自己跑，不用 eframe）。
    pub ctx: eframe::egui::Context,
    /// 适配器名字（诊断用）。
    adapter_name: String,
    config: wgpu::SurfaceConfiguration,
}

impl Host {
    /// 用已有窗口建宿主。
    pub fn new(hwnd: HWND, width: u32, height: u32) -> Result<Host, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| "没有可用的图形适配器".to_string())?;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("lingqi-render"),
                required_features: wgpu::Features::empty(),
                required_limits: small_limits(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Default::default()
            },
            None,
        ))
        .map_err(|error| format!("创建渲染设备：{error}"))?;

        let (surface, target) = create_surface(&instance, &adapter, hwnd)?;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        configure(&surface, &device, &config, &target)?;
        // 渲染器要先建好：Host 的字段按顺序求值，先把 device 移进去就借不到了。
        let renderer = Renderer::new(&device, format, true);
        let adapter_name = adapter.get_info().name.clone();

        Ok(Host {
            hwnd,
            device,
            queue,
            surface,
            target,
            renderer,
            ctx: eframe::egui::Context::default(),
            adapter_name: adapter_name,
            config,
        })
    }

    /// 窗口尺寸变化时重新配置交换链。
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if self.config.width == width && self.config.height == height {
            return Ok(());
        }
        self.config.width = width;
        self.config.height = height;
        configure(&self.surface, &self.device, &self.config, &self.target)
    }

    /// 把字体图集的变化同步给渲染器。
    pub fn sync_fonts(&mut self) {
        if let Some(delta) = self.ctx.fonts(|fonts| fonts.font_image_delta()) {
            self.renderer
                .update_texture(&self.device, &self.queue, epaint::TextureId::Managed(0), &delta);
        }
    }

    /// 把这一帧的原语画到交换链并呈现。
    pub fn present(&mut self, primitives: &[epaint::ClippedPrimitive], pixels_per_point: f32) -> Result<(), String> {
        let frame = self
            .surface
            .get_current_texture()
            .map_err(|error| format!("取交换链当前帧：{error}"))?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let screen = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point,
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // 清成全透明：岛体之外的像素必须真的透明。
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.renderer
                .render(&mut pass.forget_lifetime(), primitives, &screen);
        }
        self.queue.submit([encoder.finish()]);
        frame.present();
        Ok(())
    }

    /// 渲染器实例（测试与诊断用）。
    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    /// 适配器名字（写诊断用）。
    pub fn adapter_name(&self) -> String {
        self.adapter_name.clone()
    }
}
