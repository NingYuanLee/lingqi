//! 渲染内核的对拍测试：灵栖自写的绘制内核与自写渲染器，对上 egui-wgpu 的结果。
//!
//! 两层验证：
//!
//! 1. **细分对拍**：同一组形状，egui 的 `Context::tessellate` 与灵栖的
//!    `ShapeTessellator` 必须给出完全一样的渲染原语（顶点、索引、裁剪矩形）。
//! 2. **像素对拍**：同一组原语，分别交给 egui-wgpu 的渲染器与灵栖的渲染器，
//!    读回两张离屏纹理逐像素比较（允许 1/255 的舍入差异）。

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use epaint::{ClippedShape, PathShape, PathStroke};

/// 造一组覆盖各类图元的形状：圆角矩形、圆、折线。
fn sample_shapes() -> Vec<ClippedShape> {
    vec![
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
        ClippedShape {
            clip_rect: Rect::EVERYTHING,
            shape: egui::Shape::line_segment(
                [Pos2::new(12.0, 12.0), Pos2::new(88.0, 48.0)],
                Stroke::new(2.0, Color32::WHITE),
            ),
        },
    ]
}

/// 文字形状：需要字体图集，单独放。
fn text_shape(ctx: &egui::Context) -> ClippedShape {
    let galley = ctx.fonts(|f| f.layout_no_wrap("灵栖 Lingqi".into(), FontId::proportional(18.0), Color32::WHITE));
    ClippedShape {
        clip_rect: Rect::EVERYTHING,
        shape: egui::Shape::galley(Pos2::new(12.0, 170.0), galley, Color32::WHITE),
    }
}

/// 断言两边的原语完全一致。
fn assert_same_primitives(label: &str, actual: &[epaint::ClippedPrimitive], expected: &[epaint::ClippedPrimitive]) {
    assert_eq!(actual.len(), expected.len(), "{label}：原语数量应当一致");
    for (i, (a, b)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(a.clip_rect, b.clip_rect, "{label}：第 {i} 个原语的裁剪矩形不一致");
        let (am, bm) = match (&a.primitive, &b.primitive) {
            (epaint::Primitive::Mesh(am), epaint::Primitive::Mesh(bm)) => (am, bm),
            _ => panic!("{label}：第 {i} 个原语类型不一致"),
        };
        assert_eq!(am.texture_id, bm.texture_id, "{label}：第 {i} 个原语的纹理不一致");
        assert_eq!(am.indices, bm.indices, "{label}：第 {i} 个原语的索引不一致");
        assert_eq!(am.vertices, bm.vertices, "{label}：第 {i} 个原语的顶点不一致");
    }
}

#[test]
fn tessellation_matches_egui_exactly() {
    let ctx = egui::Context::default();
    // egui 要先跑过一帧才有字体（Context::run 之前字体表是空的）。
    ctx.run(Default::default(), |_| {});
    let pixels_per_point = 1.0;

    // egui 自己细分（内部用它的字体图集与圆盘）。
    let shapes = sample_shapes();
    let expected = ctx.tessellate(shapes.clone(), pixels_per_point);

    // 灵栖的绘制内核：用同一份图集与同样的参数。
    let font_tex_size = ctx.fonts(|f| f.font_image_size());
    let atlas = ctx.fonts(|f| f.texture_atlas());
    let mut tessellator = lingqi::render::ShapeTessellator::new(pixels_per_point, font_tex_size, &atlas.lock());
    let actual = tessellator.tessellate(Rect::EVERYTHING, shapes);

    assert_same_primitives("图形", &actual, &expected);
}

#[test]
fn text_tessellation_matches_egui_exactly() {
    let ctx = egui::Context::default();
    ctx.run(Default::default(), |_| {});
    let shape = text_shape(&ctx);
    let expected = ctx.tessellate(vec![shape.clone()], 1.0);

    let font_tex_size = ctx.fonts(|f| f.font_image_size());
    let atlas = ctx.fonts(|f| f.texture_atlas());
    let mut tessellator = lingqi::render::ShapeTessellator::new(1.0, font_tex_size, &atlas.lock());
    let actual = tessellator.tessellate(Rect::EVERYTHING, vec![shape]);

    assert_same_primitives("文字", &actual, &expected);
}
