//! DirectComposition 透明交换链。
//!
//! 移植自 `vendor/egui-wgpu/src/winit.rs`（MIT / Apache-2.0，已在第三方声明里）：
//! 在同一块 DXGI 适配器上建 D3D11 设备，换取 `IDXGIDevice` →
//! `DCompositionCreateDevice2` → visual → target，再把 visual 交给 wgpu-hal
//! 做交换链。岛体的四角因此是真正的透明，而不是「画成透明色」。
//!
//! 与上游的差别：不依赖 winit 的窗口，直接收 HWND——自研后端自己建窗口。

use windows::core::Interface as _;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{D3D11CreateDevice, ID3D11Device, D3D11_SDK_VERSION};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDevice, IDCompositionTarget,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;

/// DComp 的合成目标：持有设备与 target，析构时按正确顺序释放。
pub struct CompositionTarget {
    /// 目标窗口。
    pub hwnd: HWND,
    device: IDCompositionDevice,
    #[allow(dead_code)]
    target: IDCompositionTarget,
}

impl CompositionTarget {
    /// 提交合成树。
    pub fn commit(&self) -> Result<(), String> {
        unsafe { self.device.Commit() }
            .map_err(|error| format!("DirectComposition Commit：{error}"))
    }
}

/// 为窗口创建 DComp 交换链。
///
/// `instance` 与 `adapter` 必须是渲染器用的那一个（交换链要挂在同一块适配器上）。
pub fn create_surface(
    instance: &wgpu::Instance,
    adapter: &wgpu::Adapter,
    hwnd: HWND,
) -> Result<(wgpu::Surface<'static>, CompositionTarget), String> {
    // 用同一 DXGI 适配器创建 D3D11 设备：有些驱动无法从 D3D12 设备反查 IDXGIDevice。
    let dxgi_adapter = unsafe {
        adapter.as_hal::<wgpu::hal::api::Dx12, _, _>(|hal| {
            hal.map(|adapter| adapter.raw_dxgi_adapter().clone())
        })
    }
    .ok_or_else(|| "当前适配器未使用 Direct3D 12，无法创建 DComp 交换链".to_string())?;

    let mut d3d11: Option<ID3D11Device> = None;
    unsafe {
        D3D11CreateDevice(
            &dxgi_adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            None,
            Default::default(),
            None,
            D3D11_SDK_VERSION,
            Some(&mut d3d11),
            None,
            None,
        )
    }
    .map_err(|error| format!("D3D11CreateDevice：{error}"))?;
    let d3d11 = d3d11.ok_or_else(|| "D3D11CreateDevice 未返回设备".to_string())?;

    let dxgi_device: IDXGIDevice = d3d11
        .cast()
        .map_err(|error| format!("IDXGIDevice：{error}"))?;
    let device: IDCompositionDevice = unsafe { DCompositionCreateDevice2(&dxgi_device) }
        .map_err(|error| format!("DCompositionCreateDevice2：{error}"))?;
    let visual = unsafe { device.CreateVisual() }
        .map_err(|error| format!("CreateVisual：{error}"))?;
    let target = unsafe { device.CreateTargetForHwnd(hwnd, true) }
        .map_err(|error| format!("CreateTargetForHwnd：{error}"))?;
    unsafe { target.SetRoot(&visual) }
        .map_err(|error| format!("SetRoot：{error}"))?;

    // wgpu-hal 持有 visual 的 COM 引用；Surface 先于 target 释放。
    let surface = unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CompositionVisual(
            visual.as_raw(),
        ))
    }
    .map_err(|error| format!("创建 DComp 交换链：{error}"))?;

    Ok((
        surface,
        CompositionTarget { hwnd, device, target },
    ))
}

/// 配置交换链并提交合成树。
pub fn configure(
    surface: &wgpu::Surface<'_>,
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    target: &CompositionTarget,
) -> Result<(), String> {
    // wgpu 的错误作用域：换链配置的问题要在这里抓出来，不然只会在屏幕上黑一块。
    for filter in [
        wgpu::ErrorFilter::Validation,
        wgpu::ErrorFilter::Internal,
        wgpu::ErrorFilter::OutOfMemory,
    ] {
        device.push_error_scope(filter);
    }
    surface.configure(device, config);
    let mut error = None;
    for _ in 0..3 {
        error = error.or(pollster::block_on(device.pop_error_scope()));
    }
    if let Some(error) = error {
        return Err(format!("配置 DComp 交换链：{error}"));
    }
    target.commit()
}
