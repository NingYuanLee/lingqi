//! 灵栖的原生状态、矢量表情与表情图集播放。
use crate::anim::{approach, lerp, smoothstep};
use crate::paint::{Card, H};
use crate::theme::{self, W};
use eframe::egui::Color32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PetState {
    Awake,
    Curious,
    Waking,
    Happy,
    Satisfied,
    Worried,
    Focused,
    Dozing,
    Sleeping,
}

#[derive(Clone, Copy)]
struct Eye {
    height: f32,
    spread: f32,
    curve: f32,
    y: f32,
    tilt: f32,
}

impl Eye {
    fn open(height: f32) -> Self { Self { height, spread: 0.0, curve: 0.0, y: 0.0, tilt: 0.0 } }
    fn arc(spread: f32, curve: f32) -> Self { Self { spread, curve, ..Self::open(0.0) } }
    fn follow(&mut self, goal: Self, dt: f32) {
        self.height = approach(self.height, goal.height, 17.0, dt);
        self.spread = approach(self.spread, goal.spread, 17.0, dt);
        self.curve = approach(self.curve, goal.curve, 17.0, dt);
        self.y = approach(self.y, goal.y, 12.0, dt);
        self.tilt = approach(self.tilt, goal.tilt, 12.0, dt);
    }
}

pub(crate) struct Pet {
    pub state: PetState,
    pub reaction: Option<(PetState, f32)>,
    pub waking_until: f64,
    pub last_idle: f64,
    pub hovered: bool,
    pub worried: bool,
    pub expressions: crate::pet_expressions::Expressions,
    age: f32,
    eyes: [Eye; 2],
    gaze: [f32; 2],
    y: f32,
}

impl Default for Pet {
    fn default() -> Self {
        Self { state: PetState::Awake, reaction: None, waking_until: 0.0, last_idle: 0.0,
            hovered: false, worried: false, expressions:Default::default(), age: 0.0, eyes: [Eye::open(4.4); 2], gaze: [0.0; 2], y: 0.0 }
    }
}

impl Pet {
    pub fn react(&mut self, state: PetState) { self.reaction = Some((state, self.expressions.react(state))); self.age = 0.0; }
    /// 反馈只在实际可见时计时，避免用户还在面板里操作时就播完。
    pub fn advance(&mut self, dt: f32, visible: bool) {
        if visible {
            if let Some((_, remaining)) = &mut self.reaction {
                *remaining -= dt;
                if *remaining <= 0.0 { self.reaction = None; }
            }
        }
    }

    pub fn step(&mut self, state: PetState, t: f64, dt: f32, gaze: [f32; 2], visible: bool) {
        self.expressions.step(state,dt,visible);
        if state != self.state { self.state = state; self.age = 0.0; }
        if visible { self.age += dt; }
        let age = self.age;
        let mut eyes = [Eye::open(4.4); 2];
        let mut y = (t as f32 * 1.7).sin() * 0.35;
        let mut look = gaze;
        match state {
            PetState::Awake => {},
            PetState::Curious => {
                eyes[0].height = 5.6; eyes[1].height = 6.3;
                eyes[0].y = 0.6; eyes[1].y = -0.6;
            },
            PetState::Waking => {
                eyes[0].height = lerp(0.0, 4.4, smoothstep(0.1, 0.65, age));
                eyes[1].height = lerp(0.0, 4.4, smoothstep(0.4, 1.1, age));
                y = 1.0 - smoothstep(0.0, 1.2, age); look = [0.0; 2];
            },
            PetState::Happy => {
                eyes = [Eye::arc(2.6, -1.8); 2];
                y = -(age * 7.5).sin().abs() * 2.0 * (-age * 0.65).exp();
            },
            PetState::Satisfied => {
                eyes = [Eye::arc(2.2, -1.2); 2];
                y = (age * 8.0).sin() * 1.2 * (-age * 0.8).exp();
            },
            PetState::Worried => {
                eyes[0] = Eye { spread: -0.6, tilt: -0.7, y: 0.4, ..Eye::open(2.8) };
                eyes[1] = Eye { spread: 0.6, tilt: 0.7, y: 1.0, ..Eye::open(1.8) };
            },
            PetState::Focused => { eyes = [Eye::open(1.8); 2]; look = [gaze[0] * 0.2, 0.0]; y = 0.0; },
            PetState::Dozing => { eyes = [Eye::open(0.6); 2]; y = 1.0; look = [0.0; 2]; },
            PetState::Sleeping => { eyes = [Eye::arc(2.4, 1.0); 2]; y = 0.8 + (t as f32 * 1.2).sin() * 0.4; look = [0.0; 2]; },
        }
        if self.expressions.current().is_some() {eyes=[Eye::open(4.4);2];look=[0.0;2];y=0.0;}
        if self.expressions.current().is_none() && matches!(state, PetState::Awake | PetState::Curious | PetState::Focused | PetState::Worried) {
            let start = 2.0 + crate::anim::hash01((t / 5.6).floor() as u64) * 2.7;
            let blink = ((t % 5.6) as f32 - start) / 0.18;
            if (0.0..1.0).contains(&blink) {
                for eye in &mut eyes { eye.height *= 1.0 - (blink * std::f32::consts::PI).sin(); }
            }
        }
        let eye_dt=dt;
        for (eye, goal) in self.eyes.iter_mut().zip(eyes) { eye.follow(goal, eye_dt); }
        for (axis, goal) in self.gaze.iter_mut().zip(look) { *axis = approach(*axis, goal, 10.0, dt); }
        self.y = approach(self.y, y, 14.0, dt);
    }

    fn draw_face(&mut self, c: &Card, cx: f32, palette:Option<crate::skin_paint::Palette>, ink:Color32) -> bool {
        if self.expressions.draw(c,cx,palette) {return true;}
        for (i, eye) in self.eyes.iter().enumerate() {
            let x = cx + if i == 0 { -5.6 } else { 5.6 } + self.gaze[0];
            let y = 14.5 + self.y + eye.y + self.gaze[1];
            let points: Vec<_> = (0..=12).map(|n| {
                let q = n as f32 / 6.0 - 1.0;
                (x + q * eye.spread, y + q * (eye.height / 2.0 + eye.tilt) + (1.0 - q*q) * eye.curve)
            }).collect();
            let stroke=3.6;
            c.path(&points, stroke, ink);
            // 圆端点保持胶囊眼睛的轮廓。
            for p in [points[0], points[12]] { c.circle(p.0, p.1, stroke/2.0, ink); }
        }
        false
    }

    #[cfg(feature="sdk-js")]
    pub(crate) fn draw_canvas_face(&mut self,c:&Card,cx:f32,palette:Option<crate::skin_paint::Palette>,ink:Color32) {
        self.draw_face(c,cx,palette,ink);
    }

    pub fn draw(&mut self, c: &Card, cx: f32, t: f64, palette:Option<crate::skin_paint::Palette>) {
        if self.draw_face(c,cx,palette,theme::WHITE) {return;}
        match self.state {
            PetState::Dozing | PetState::Sleeping => {
                let bob = ((t as f32 * 1.4).sin() + 1.0) * 0.7;
                c.text(cx+14.0, 13.0-bob, "z", 7.0, W::Bold, Color32::from_rgb(170,160,255), H::Left);
                if self.state == PetState::Sleeping { c.text(cx+19.0, 9.0-bob, "z", 5.5, W::Bold, Color32::from_rgb(170,160,255), H::Left); }
            },
            PetState::Happy => {
                let k = smoothstep(0.0, 0.2, self.age) * (1.0-smoothstep(1.5, 2.5, self.age));
                let c = c.faded(k);
                for (dx, y, r) in [(-18.0, 9.0, 2.1), (18.0, 8.0, 2.8)] {
                    c.seg((cx+dx-r,y),(cx+dx+r,y),1.0,theme::GREEN);
                    c.seg((cx+dx,y-r),(cx+dx,y+r),1.0,theme::GREEN);
                }
            },
            PetState::Satisfied => crate::paint::droplet(c,cx+18.0,12.0,5.0,theme::CYAN),
            PetState::Worried => {
                c.fill(cx+17.0,8.0,1.6,4.2,0.8,theme::YELLOW);
                c.circle(cx+17.8,14.5,0.9,theme::YELLOW);
            },
            _ => {},
        }
    }

}
