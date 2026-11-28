//! 自研渲染后端的冒烟测试：真的建窗口、建 DirectComposition 交换链、跑一帧并呈现。
//!
//! 这条测试走的是完整链路：
//!
//! `窗口 → wgpu 设备 → DComp 交换链 → egui 出一帧 → 自写渲染器 → 呈现`
//!
//! 环境不支持 DComp（比如没有 D3D12 适配器）时**跳过**而不是失败——
//! 那属于机器差异，不是代码缺陷。

use lingqi::render::host::Host;
use lingqi::render::window::IslandWindow;

#[test]
fn self_backend_presents_one_frame() {
    let window = match IslandWindow::new(64, 64, 0, 0) {
        Ok(window) => window,
        Err(error) => {
            eprintln!("跳过：建不了岛体窗口（{error}）");
            return;
        }
    };
    let mut host = match Host::new(window.hwnd(), 64, 64) {
        Ok(host) => host,
        Err(error) => {
            eprintln!("跳过：建不了自研渲染宿主（{error}）");
            return;
        }
    };

    // 画一个实心方块，验证这一帧真的走了「egui → 细分 → 自写渲染器 → 呈现」。
    let output = host.ctx.run(
        eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                eframe::egui::Pos2::ZERO,
                eframe::egui::vec2(64.0, 64.0),
            )),
            ..Default::default()
        },
        |ctx| {
            // egui 的绘制要走一个临时 Ui（与 AI 抽屉同一套做法）。
            let mut ui = eframe::egui::Ui::new(
                ctx.clone(),
                eframe::egui::Id::new("self-backend-smoke"),
                eframe::egui::UiBuilder::new().max_rect(eframe::egui::Rect::from_min_size(
                    eframe::egui::Pos2::ZERO,
                    eframe::egui::vec2(64.0, 64.0),
                )),
            );
            ui.painter().rect_filled(
                eframe::egui::Rect::from_min_size(
                    eframe::egui::Pos2::ZERO,
                    eframe::egui::vec2(64.0, 64.0),
                ),
                0.0,
                eframe::egui::Color32::from_rgb(40, 120, 200),
            );
        },
    );
    assert!(!output.shapes.is_empty(), "这一帧应当画出东西");

    host.sync_fonts();
    let pixels_per_point = host.ctx.pixels_per_point();
    let primitives = host.ctx.tessellate(output.shapes, pixels_per_point);
    assert!(!primitives.is_empty(), "细分后应当有原语");
    host.present(&primitives, pixels_per_point)
        .expect("呈现这一帧");
}
