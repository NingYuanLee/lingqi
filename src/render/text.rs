//! 字体：排版与字形图集。
//!
//! 渲染内核只需要 epaint 的 `Fonts` 提供三样东西：
//!
//! - 排版结果（`Galley`）；
//! - 字形图集的增量（新增或变化的字形，用于局部上传）；
//! - 图集尺寸与 `prepared_discs`——文字的 uv 要按图集尺寸归一化，圆形抗锯齿也要
//!   靠 `prepared_discs`，少了它圆会变成多边形。

use std::sync::Arc;

use epaint::text::{FontDefinitions, FontId, Galley, LayoutJob};
use epaint::{Color32, ImageDelta};

/// 字体状态：排版 + 字形图集。
///
/// 与 egui 的 `Context` 里的字体是同一套机制，但独立持有，便于渲染内核单独使用
/// （例如离屏渲染或像素对拍）。
pub struct TextState {
    fonts: epaint::text::Fonts,
    pixels_per_point: f32,
    max_texture_side: usize,
}

impl TextState {
    /// 建立字体状态。`definitions` 一般用 egui 的默认字体定义。
    pub fn new(definitions: FontDefinitions, pixels_per_point: f32, max_texture_side: usize) -> Self {
        let fonts = epaint::text::Fonts::new(pixels_per_point, max_texture_side, definitions);
        Self { fonts, pixels_per_point, max_texture_side }
    }

    /// 每帧开始：像素比变化或图集快满时重建图集。
    pub fn begin_pass(&mut self) {
        self.fonts.begin_pass(self.pixels_per_point, self.max_texture_side);
    }

    /// 排版一段可换行的文字。
    pub fn layout(&self, text: String, font_id: FontId, color: Color32, wrap_width: f32) -> Arc<Galley> {
        self.fonts.layout(text, font_id, color, wrap_width)
    }

    /// 排版一段不换行的文字。
    pub fn layout_no_wrap(&self, text: String, font_id: FontId, color: Color32) -> Arc<Galley> {
        self.fonts.layout_no_wrap(text, font_id, color)
    }

    /// 排版一个布局任务（混排、居中、指定颜色）。
    pub fn layout_job(&self, job: LayoutJob) -> Arc<Galley> {
        self.fonts.layout_job(job)
    }

    /// 字形图集的增量：只有新增或变化的字形，没有则返回 None。
    pub fn font_image_delta(&self) -> Option<ImageDelta> {
        self.fonts.font_image_delta()
    }

    /// 字形图集尺寸：文字 uv 归一化要用它。
    pub fn font_image_size(&self) -> [usize; 2] {
        self.fonts.font_image_size()
    }

    /// 字形图集：里面既有 uv 归一化要用的尺寸，也有圆形抗锯齿要的预制圆盘。
    ///
    /// `PreparedDisc` 在 epaint 里没有公开导出，只能通过图集拿。
    pub fn texture_atlas(&self) -> std::sync::Arc<epaint::mutex::Mutex<epaint::TextureAtlas>> {
        self.fonts.texture_atlas()
    }

    /// 图集占用率：超过 0.8 时 egui 会重建图集。
    pub fn atlas_fill_ratio(&self) -> f32 {
        self.fonts.font_atlas_fill_ratio()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epaint::text::Fonts;

    #[test]
    fn font_image_size_matches_the_atlas() {
        let fonts = Fonts::new(1.0, 2048, FontDefinitions::default());
        assert!(fonts.font_image_size().iter().all(|v| *v > 0));
    }

    #[test]
    fn layout_produces_a_galley_with_the_text() {
        let mut state = TextState::new(FontDefinitions::default(), 1.0, 2048);
        state.begin_pass();
        let galley = state.layout_no_wrap("灵栖".into(), FontId::proportional(12.0), Color32::WHITE);
        assert!(!galley.text().is_empty());
    }
}
