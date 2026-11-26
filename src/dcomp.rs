//! DirectComposition 透明直绘（仅 Windows）。
//!
//! 为什么重要：普通窗口只能不透明，普通 GPU 表面在 Win32 上也只支持 `Opaque` 合成。
//! 走 DirectComposition 才能把岛体直接画进桌面合成器：
//! 不需要每帧把画面读回 CPU 再交给分层窗口，空闲占用与动画延迟都低得多。
//!
//! 上游把这条路径锁在 `sdk-material` 实验特性后面；灵栖把它做成正经能力，
//! 由图形后端在启动时决定开不开，失败时自动退回普通 DX12。

/// 启动前设置是否使用透明交换链。
pub fn configure(options: &mut eframe::NativeOptions, enabled: bool) {
    options.wgpu_options.direct_composition = enabled;
}

/// 取回初始化阶段的失败原因（若有）。
pub fn take_error() -> Option<String> {
    eframe::egui_wgpu::winit::take_direct_composition_error()
}

/// 运行阶段是否因为透明交换链失败而需要退回。
pub fn report_run_failure(result: &eframe::Result<()>) -> bool {
    let _ = take_error();
    if let Err(eframe::Error::Wgpu(eframe::egui_wgpu::WgpuError::DirectComposition(error))) = result {
        crate::resources::append_log(
            &crate::model::config_dir().join("diagnostics"),
            "renderer.log",
            &format!("透明交换链运行失败，退回普通绘制：{error}"),
        );
        true
    } else {
        false
    }
}
