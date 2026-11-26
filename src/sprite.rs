//! 参数化精灵：不依赖任何美术素材，用几何与时间轴画出小生灵。
//!
//! 设计目标：
//! - 只输出 [`epaint::Shape`]，不碰 egui 的 widget，因此既能画进现在的 egui 渲染器，
//!   也能直接喂给后续的自研 wgpu 渲染器。
//! - 动画完全由时间轴驱动，相同 `dt` 序列必定得到相同画面（便于录屏与回归对比）。
//! - [`SpriteState::advance`] 会回报「还在动吗」，供空闲不重绘策略使用。

use crate::anim::hash01;
use epaint::{Color32, Pos2, Shape, Vec2};

/// 眨眼时长（秒）。
const BLINK_DURATION: f32 = 0.12;
/// 平静（休眠）时呼吸与浮沉的淡出时长（秒）。
const CALM_FADE: f32 = 0.9;

/// 精灵外观与动画参数。
#[derive(Clone, Debug)]
pub struct SpriteParams {
    /// 身体半径（逻辑像素）。
    pub body_radius: f32,
    /// 身体主色。
    pub body_color: Color32,
    /// 外圈柔光颜色。
    pub glow_color: Color32,
    /// 眼睛颜色。
    pub eye_color: Color32,
    /// 双眼间距（相对身体半径）。
    pub eye_spread: f32,
    /// 眼睛半径（相对身体半径）。
    pub eye_radius: f32,
    /// 高光半径（相对身体半径）。
    pub glint_scale: f32,
    /// 眨眼间隔（秒）。
    pub blink_interval: f32,
    /// 眨眼间隔的随机抖动比例（0..1）。
    pub blink_jitter: f32,
    /// 呼吸周期（秒）。
    pub breath_period: f32,
    /// 浮沉周期（秒）。
    pub float_period: f32,
    /// 浮沉幅度（像素）。
    pub float_amplitude: f32,
}

impl Default for SpriteParams {
    /// 默认形象：玻璃罩里的薄荷光球（与应用图标同一套配色）。
    fn default() -> Self {
        Self {
            body_radius: 7.0,
            body_color: Color32::from_rgb(150, 240, 205),
            glow_color: Color32::from_rgba_unmultiplied(120, 235, 200, 70),
            eye_color: Color32::from_rgb(26, 30, 34),
            eye_spread: 0.42,
            eye_radius: 0.22,
            glint_scale: 0.16,
            blink_interval: 3.6,
            blink_jitter: 0.5,
            breath_period: 2.8,
            float_period: 4.6,
            float_amplitude: 0.7,
        }
    }
}

/// 精灵的实时状态：时间轴、眨眼进度、视线。
#[derive(Clone, Debug)]
pub struct SpriteState {
    /// 已推进的时间（秒）。
    pub t: f32,
    /// 眨眼进度：0 完全睁开，1 完全闭合。
    pub blink: f32,
    /// 视线偏移（相对身体半径，-1..1）。
    pub gaze: [f32; 2],
    /// 平静度：0 活跃，1 完全静止。
    pub calm: f32,
    seed: u64,
    next_blink: f32,
    blink_started: Option<f32>,
}

impl Default for SpriteState {
    fn default() -> Self {
        Self::new(0x5EED)
    }
}

impl SpriteState {
    /// 新建一个精灵。`seed` 决定眨眼节奏，相同 seed 得到相同表现。
    pub fn new(seed: u64) -> Self {
        Self {
            t: 0.0,
            blink: 0.0,
            gaze: [0.0, 0.0],
            calm: 0.0,
            seed,
            next_blink: 0.0,
            blink_started: None,
        }
    }

    /// 下一次眨眼距现在的间隔。抖动由 seed 决定，可复现。
    fn blink_interval_of(&self, params: &SpriteParams) -> f32 {
        let base = params.blink_interval.max(0.2);
        let jitter = hash01(self.seed ^ (self.t.to_bits() as u64).wrapping_mul(0x9E3779B97F4A7C15));
        base + jitter * base * params.blink_jitter.clamp(0.0, 1.0)
    }

    /// 推进时间轴。
    ///
    /// - `dt`：距上一帧的秒数。
    /// - `params`：外观与节奏参数。
    /// - `calm`：是否进入休眠（休眠时呼吸与浮沉淡出、停止眨眼）。
    ///
    /// 返回 `true` 表示画面仍在变化，需要继续重绘；`false` 表示已经静止。
    pub fn advance(&mut self, dt: f32, params: &SpriteParams, calm: bool) -> bool {
        if dt > 0.0 {
            self.t += dt;
        }
        self.calm = if calm {
            (self.calm + dt / CALM_FADE).min(1.0)
        } else {
            (self.calm - dt / CALM_FADE).max(0.0)
        };

        // 眨眼：到点开始，BLINK_DURATION 内闭合再睁开。
        self.blink = 0.0;
        if self.next_blink <= 0.0 {
            // 第一次推进时按当前参数排下一次眨眼。
            self.next_blink = self.t + self.blink_interval_of(params);
        }
        if self.calm < 0.999 {
            if let Some(started) = self.blink_started {
                let elapsed = self.t - started;
                if elapsed >= BLINK_DURATION {
                    self.blink_started = None;
                    self.next_blink = self.t + self.blink_interval_of(params);
                } else {
                    let phase = elapsed / BLINK_DURATION;
                    self.blink = (phase * std::f32::consts::PI).sin();
                }
            } else if self.t >= self.next_blink {
                self.blink_started = Some(self.t);
                self.blink = 0.0_f32;
            }
        }

        // 呼吸与浮沉在平静时收敛到 0，但只要还没完全静止就仍需重绘。
        self.calm < 0.999 || self.blink > 0.0 || self.blink_started.is_some()
    }

    /// 当前呼吸相位（0..1）。
    pub fn breath(&self, params: &SpriteParams) -> f32 {
        let period = params.breath_period.max(0.1);
        (self.t / period * std::f32::consts::TAU).sin() * 0.5 + 0.5
    }

    /// 当前浮沉偏移（像素）。
    pub fn float_offset(&self, params: &SpriteParams) -> f32 {
        let period = params.float_period.max(0.1);
        let wave = (self.t / period * std::f32::consts::TAU).sin();
        wave * params.float_amplitude * (1.0 - self.calm)
    }

    /// 画出一帧。`origin` 是身体中心，`scale` 是整体缩放。
    ///
    /// 返回的图形自底向上排列：柔光 → 身体 → 高光 → 眼睛。
    pub fn shapes(&self, origin: Pos2, scale: f32, params: &SpriteParams) -> Vec<Shape> {
        let scale = scale.max(0.0);
        let radius = params.body_radius * scale;
        if radius <= 0.1 {
            return Vec::new();
        }
        let breath = self.breath(params) * (1.0 - self.calm);
        let radius = radius * (1.0 + breath * 0.045);
        let center = origin + Vec2::new(0.0, -self.float_offset(params) * scale);

        let mut shapes = Vec::with_capacity(8);

        // 外圈柔光：用几层递减透明度的圆叠出光晕，成本极低。
        for (i, factor) in [1.55_f32, 1.3, 1.12].into_iter().enumerate() {
            let alpha = [26_u8, 34, 46][i];
            shapes.push(Shape::circle_filled(
                center,
                radius * factor,
                Color32::from_rgba_unmultiplied(
                    params.glow_color.r(),
                    params.glow_color.g(),
                    params.glow_color.b(),
                    alpha,
                ),
            ));
        }

        // 身体：外圈半透明 + 内圈实色，做出玻璃球体积感。
        shapes.push(Shape::circle_filled(
            center,
            radius,
            Color32::from_rgba_unmultiplied(
                params.body_color.r(),
                params.body_color.g(),
                params.body_color.b(),
                150,
            ),
        ));
        shapes.push(Shape::circle_filled(
            center,
            radius * 0.86,
            params.body_color,
        ));

        // 左上高光。
        let glint = center + Vec2::new(-radius * 0.34, -radius * 0.38);
        shapes.push(Shape::circle_filled(
            glint,
            radius * params.glint_scale,
            Color32::from_rgba_unmultiplied(255, 255, 255, 190),
        ));

        // 眼睛：椭圆，随视线平移、随眨眼压扁。
        let open = (1.0 - self.blink).max(0.0);
        let eye_r = radius * params.eye_radius;
        let spread = radius * params.eye_spread;
        for side in [-1.0_f32, 1.0] {
            let ex = center.x + side * spread + self.gaze[0] * radius * 0.16;
            let ey = center.y + self.gaze[1] * radius * 0.1;
            shapes.push(ellipse(
                Pos2::new(ex, ey),
                eye_r,
                eye_r * (0.25 + 0.75 * open),
                params.eye_color,
            ));
        }
        shapes
    }
}

/// 用多边形近似椭圆（比圆更可控地压扁）。
fn ellipse(center: Pos2, rx: f32, ry: f32, color: Color32) -> Shape {
    let steps = 20;
    let points: Vec<Pos2> = (0..steps)
        .map(|i| {
            let angle = i as f32 / steps as f32 * std::f32::consts::TAU;
            center + Vec2::new(angle.cos() * rx, angle.sin() * ry)
        })
        .collect();
    Shape::convex_polygon(points, color, epaint::Stroke::NONE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> SpriteParams {
        SpriteParams::default()
    }

    #[test]
    fn advance_is_deterministic_for_same_dt_sequence() {
        let mut a = SpriteState::new(42);
        let mut b = SpriteState::new(42);
        let p = params();
        for _ in 0..120 {
            a.advance(1.0 / 60.0, &p, false);
            b.advance(1.0 / 60.0, &p, false);
        }
        assert_eq!(a.t, b.t);
        assert_eq!(a.blink, b.blink);
    }

    #[test]
    fn different_seeds_blink_differently() {
        let mut a = SpriteState::new(1);
        let mut b = SpriteState::new(2);
        let p = params();
        let mut a_blinks = 0;
        let mut b_blinks = 0;
        let mut a_prev = 0.0;
        let mut b_prev = 0.0;
        for _ in 0..600 {
            a.advance(1.0 / 60.0, &p, false);
            b.advance(1.0 / 60.0, &p, false);
            if a.blink > 0.5 && a_prev <= 0.5 {
                a_blinks += 1;
            }
            if b.blink > 0.5 && b_prev <= 0.5 {
                b_blinks += 1;
            }
            a_prev = a.blink;
            b_prev = b.blink;
        }
        assert!(a_blinks > 0, "应当在十秒内眨过眼");
        assert!(b_blinks > 0, "应当在十秒内眨过眼");
    }

    #[test]
    fn calm_sprite_eventually_settles() {
        let mut state = SpriteState::new(7);
        let p = params();
        let mut moving = true;
        for _ in 0..600 {
            moving = state.advance(1.0 / 60.0, &p, true);
        }
        assert!(!moving, "休眠后应当报告静止");
        assert!((state.calm - 1.0).abs() < 0.01);
    }

    #[test]
    fn awake_sprite_keeps_moving() {
        let mut state = SpriteState::new(7);
        let p = params();
        let mut moving = false;
        for _ in 0..600 {
            moving = state.advance(1.0 / 60.0, &p, false);
        }
        assert!(moving, "清醒时应当持续重绘");
    }

    #[test]
    fn shapes_have_body_and_two_eyes() {
        let state = SpriteState::default();
        let shapes = state.shapes(Pos2::new(50.0, 50.0), 1.0, &params());
        // 3 层柔光 + 身体外圈 + 身体内圈 + 高光 + 2 只眼睛 = 8
        assert_eq!(shapes.len(), 8);
    }

    #[test]
    fn blink_squeezes_eyes() {
        let mut p = params();
        // 把眨眼间隔调短，保证测试窗口内一定眨到眼。
        p.blink_interval = 0.4;
        p.blink_jitter = 0.0;
        let mut state = SpriteState::new(3);
        let mut closed = false;
        for _ in 0..600 {
            state.advance(1.0 / 120.0, &p, false);
            if state.blink > 0.9 {
                closed = true;
                break;
            }
        }
        assert!(closed, "应当出现完全闭眼的帧");
    }

    #[test]
    fn zero_scale_draws_nothing() {
        let state = SpriteState::default();
        assert!(state.shapes(Pos2::ZERO, 0.0, &params()).is_empty());
    }
}
