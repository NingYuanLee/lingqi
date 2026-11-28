//! 灵栖：Windows 顶部「灵动岛」。
//!
//! - `app` / `pages`：与平台无关的界面与动效（外壳与录制器共用）
//! - `anim` / `paint` / `shape` / `theme`：弹簧、矢量绘制、面板轮廓、配色字体
//! - `model` / `lyrics` / `smtc` / `icons`：数据、歌词、系统媒体、文件图标
//! - `native` / `tray`：仅 Windows 的窗口区域、命中穿透与托盘

pub mod display_layout;
pub mod exe_info;
pub mod diagnostics_tool;
pub mod rtl_text;
#[cfg(windows)]
#[cfg(windows)]
#[cfg(windows)]
pub mod poll_clock;
pub mod resources;
pub mod ai;
pub mod anim;
#[cfg(windows)]
pub mod dcomp;
pub mod net;
pub mod render;
pub mod sprite;
pub mod smtc;
pub mod worldclock;
pub mod oled_shift;
pub mod app;
pub mod child_process;
mod settings_window;
pub mod glass;
pub mod skin_paint;
pub mod icons;
pub mod input;
pub mod clipboard;
pub mod notify_privacy;
pub mod side_dock;
mod task_picker;
pub mod model;
pub mod renderer;
#[cfg(all(windows, feature = "material"))]
pub mod material;
pub mod lunar_calendar;
mod weather;
mod weather_ui;
mod pages;
mod extras;
mod pet;
mod pet_expressions;
mod bar_layout;
pub mod performance;
mod performance_ui;
mod resident_settings;
mod resident_date;
mod avoidance;
mod focus_bar;
#[cfg(windows)]
pub mod backdrop;
#[cfg(windows)]
pub mod composition;
#[cfg(windows)]
pub mod layered;
#[cfg(windows)]
pub mod system;
pub mod paint;
pub mod shape;
mod fluid;
pub mod theme;
pub mod settings_theme;
mod updates;
#[cfg(windows)]
pub use updates::record_install_size;

#[cfg(windows)]
pub mod native;
#[cfg(windows)]
mod window_bounds;
#[cfg(windows)]
mod crop_front_buffer;
#[cfg(windows)]
#[cfg(windows)]
pub mod tray;

#[cfg(test)]
mod ui_tests;
#[cfg(test)]
mod preview_raster;
