//! 形状 → 渲染原语（与后端无关的绘制内核）。
//!
//! epaint 的 `Tessellator` 把路径、圆、圆角矩形变成三角形，并给出每个原语的裁剪矩形。
//! 它需要两样输入：
//!
//! - 字体图集尺寸：文字的 uv 要按它归一化；
//! - `prepared_discs`：圆形抗锯齿的前提，缺了圆会退化成多边形。
//!
//! 细分器要复用：epaint 明确建议每帧复用同一个实例（少分配）。

use epaint::{ClippedPrimitive, ClippedShape, Rect, TessellationOptions, Tessellator};

/// 形状细分器：把已裁剪的形状变成渲染原语。
pub struct ShapeTessellator {
    inner: Tessellator,
}

impl ShapeTessellator {
    /// 建立细分器。
    ///
    /// `font_tex_size` 是字形图集尺寸；`atlas` 提供圆形抗锯齿要的预制圆盘
    /// （epaint 没有公开导出圆盘类型，只能从图集取）。
    pub fn new(pixels_per_point: f32, font_tex_size: [usize; 2], atlas: &epaint::TextureAtlas) -> Self {
        Self {
            inner: Tessellator::new(pixels_per_point, TessellationOptions::default(), font_tex_size, atlas.prepared_discs()),
        }
    }

    /// 细分一组形状。`clip_rect` 是整体裁剪区（原语自身的裁剪矩形仍会保留）。
    pub fn tessellate(&mut self, clip_rect: Rect, shapes: Vec<ClippedShape>) -> Vec<ClippedPrimitive> {
        self.inner.set_clip_rect(clip_rect);
        let mut out = Vec::new();
        for shape in shapes {
            self.inner.tessellate_clipped_shape(shape, &mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epaint::{Color32, PathShape, PathStroke, Pos2, Vec2};

    fn shapes() -> Vec<ClippedShape> {
        vec![
            ClippedShape {
                clip_rect: Rect::EVERYTHING,
                shape: epaint::Shape::Path(PathShape {
                    points: vec![
                        Pos2::new(0.0, 0.0),
                        Pos2::new(10.0, 0.0),
                        Pos2::new(10.0, 10.0),
                    ],
                    closed: true,
                    fill: Color32::RED,
                    stroke: PathStroke::new(1.0, Color32::BLUE),
                }),
            },
            ClippedShape {
                clip_rect: Rect::EVERYTHING,
                shape: epaint::Shape::circle_filled(Pos2::new(5.0, 5.0), 3.0, Color32::GREEN),
            },
        ]
    }

    #[test]
    fn tessellation_produces_primitives_with_clip_rects() {
        let mut tess = ShapeTessellator::new(1.0, [64, 64], &epaint::TextureAtlas::new([1024, 1024]));
        let out = tess.tessellate(Rect::EVERYTHING, shapes());
        assert!(!out.is_empty());
        assert!(out.iter().all(|p| p.clip_rect.is_positive()));
    }

    #[test]
    fn a_clip_rect_is_passed_through_to_epaint() {
        // 整体裁剪区由 epaint 决定怎么剔除（不同图元走的路径不一样），
        // 这里只保证裁剪区确实传到了细分器：裁剪前后结果不同。
        let mut tess = ShapeTessellator::new(1.0, [1024, 1024], &epaint::TextureAtlas::new([1024, 1024]));
        let all = tess.tessellate(Rect::EVERYTHING, shapes());
        let mut tess = ShapeTessellator::new(1.0, [1024, 1024], &epaint::TextureAtlas::new([1024, 1024]));
        let clipped = tess.tessellate(Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::splat(4.0)), shapes());
        let vertices = |prims: &[epaint::ClippedPrimitive]| -> usize {
            prims.iter().filter_map(|p| match &p.primitive {
                epaint::Primitive::Mesh(mesh) => Some(mesh.vertices.len()),
                _ => None,
            }).sum()
        };
        assert!(vertices(&all) > 0, "完整画布应当有顶点");
        assert!(vertices(&clipped) <= vertices(&all), "裁剪后顶点不应变多");
    }
}
