//! 像素对拍：同一组渲染原语，分别交给 egui-wgpu 的渲染器与灵栖自写的渲染器，
//! 读回两张离屏纹理逐像素比较。
//!
//! 允许 1/255 的舍入差异（两边都会做一次 sRGB 往返，浮点顺序不同会带来最后一位的差）。

use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use epaint::{ClippedShape, PathShape, PathStroke};

const SIZE: [u32; 2] = [256, 256];

/// 覆盖圆角矩形、圆、折线、描边与文字。
fn sample_shapes(ctx: &egui::Context) -> Vec<ClippedShape> {
    let mut shapes = vec![
        ClippedShape {
            clip_rect: Rect::EVERYTHING,
            shape: egui::Shape::rect_filled(
                Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(80.0, 40.0)),
                8.0,
                Color32::from_rgb(40, 120, 200),
            ),
        },
        ClippedShape {
            clip_rect: Rect::EVERYTHING,
            shape: egui::Shape::circle_filled(Pos2::new(60.0, 90.0), 18.0, Color32::from_rgb(240, 120, 60)),
        },
        ClippedShape {
            clip_rect: Rect::EVERYTHING,
            shape: egui::Shape::Path(PathShape {
                points: vec![Pos2::new(10.0, 130.0), Pos2::new(50.0, 150.0), Pos2::new(90.0, 125.0)],
                closed: false,
                fill: Color32::TRANSPARENT,
                stroke: PathStroke::new(3.0, Color32::from_rgb(120, 240, 160)),
            }),
        },
    ];
    let galley = ctx.fonts(|f| f.layout_no_wrap("灵栖 Lingqi".into(), eframe::egui::FontId::proportional(18.0), Color32::WHITE));
    shapes.push(ClippedShape {
        clip_rect: Rect::EVERYTHING,
        shape: egui::Shape::galley(Pos2::new(12.0, 170.0), galley, Color32::WHITE),
    });
    shapes
}

/// 建一张离屏纹理。
fn target_texture(device: &wgpu::Device) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("parity-target"),
        size: wgpu::Extent3d { width: SIZE[0], height: SIZE[1], depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// 把纹理读回 CPU。
fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let bytes_per_row = SIZE[0] * 4;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("parity-readback"),
        size: (bytes_per_row * SIZE[1]) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(SIZE[1]),
            },
        },
        wgpu::Extent3d { width: SIZE[0], height: SIZE[1], depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range().to_vec();
    buffer.unmap();
    data
}

#[test]
fn pixels_match_egui_wgpu() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) {
        Some(adapter) => adapter,
        None => {
            // 没有可用显卡时跳过（例如无 GPU 的构建机）。
            eprintln!("跳过像素对拍：没有可用的显卡适配器");
            return;
        }
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
        .expect("无法创建 wgpu 设备");

    let ctx = egui::Context::default();
    ctx.run(Default::default(), |_| {});
    let shapes = sample_shapes(&ctx);
    let primitives = ctx.tessellate(shapes, 1.0);

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut egui_renderer = egui_wgpu::Renderer::new(&device, format, None, 1, true);
    let mut my_renderer = lingqi::render::Renderer::new(&device, format, true);

    // 字体图集要同时喂给两个渲染器。
    if let Some(delta) = ctx.fonts(|f| f.font_image_delta()) {
        egui_renderer.update_texture(&device, &queue, egui::TextureId::Managed(0), &delta);
        my_renderer.update_texture(&device, &queue, egui::TextureId::Managed(0), &delta);
    }

    let screen_egui = egui_wgpu::ScreenDescriptor {
        size_in_pixels: SIZE,
        pixels_per_point: 1.0,
    };
    let screen_mine = lingqi::render::ScreenDescriptor {
        size_in_pixels: SIZE,
        pixels_per_point: 1.0,
    };

    // ---- egui-wgpu 渲染到 A ----
    let texture_a = target_texture(&device);
    {
        let view_a = texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let callbacks = egui_renderer.update_buffers(&device, &queue, &mut encoder, &primitives, &screen_egui);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view_a,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            egui_renderer.render(&mut pass.forget_lifetime(), &primitives, &screen_egui);
        }
        queue.submit(callbacks);
        queue.submit([encoder.finish()]);
    }

    // ---- 灵栖渲染器渲染到 B ----
    let texture_b = target_texture(&device);
    {
        let view_b = texture_b.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        my_renderer.update_buffers(&device, &queue, &primitives, &screen_mine);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view_b,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            my_renderer.render(&mut pass.forget_lifetime(), &primitives, &screen_mine);
        }
        queue.submit([encoder.finish()]);
    }

    let pixels_a = read_back(&device, &queue, &texture_a);
    let pixels_b = read_back(&device, &queue, &texture_b);
    assert_eq!(pixels_a.len(), pixels_b.len());

    let mut worst = 0u8;
    let mut worst_at = 0usize;
    let mut differing = 0usize;
    for (i, (a, b)) in pixels_a.iter().zip(pixels_b.iter()).enumerate() {
        let d = a.abs_diff(*b);
        if d > 0 {
            differing += 1;
        }
        if d > worst {
            worst = d;
            worst_at = i;
        }
    }
    assert!(
        worst <= 1,
        "像素差异过大：最大 {worst}（位置 {worst_at}，共 {differing} 个字节不同）"
    );
}
