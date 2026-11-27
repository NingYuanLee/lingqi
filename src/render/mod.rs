//! 灵栖的渲染内核。
//!
//! 分三层，从后往前依赖：
//!
//! - [`text`]：字体与字形图集。epaint 的 `Fonts` 负责排版，字形图集提供 uv 归一化
//!   需要的尺寸与圆形抗锯齿用的 `prepared_discs`。
//! - [`tessellate`]：把形状细分成渲染原语（三角形 + 裁剪矩形）。这一层与后端无关，
//!   自写渲染器与 egui-wgpu 用的是同一份细分结果，因此可以逐像素对拍。
//! - [`renderer`]：自写的 wgpu 渲染器，把原语画到离屏纹理或交换链。
//!
//! 与 egui-wgpu 的分工：egui-wgpu 仍然负责设置窗口、输入与 UI 框架；岛体绘制可以
//! 走这里。两者共用同一套顶点布局与混合方式，输出应当逐像素一致
//! （见 `tests/render_parity.rs`）。

pub mod renderer;
pub mod tessellate;
pub mod text;

pub use renderer::{Renderer, ScreenDescriptor};
pub use tessellate::ShapeTessellator;
pub use text::TextState;
