//! 自研后端的消息循环：收窗口消息 → 跑 egui → 用自写渲染器呈现。
//!
//! 与 eframe 的循环相比只做三件事：
//!
//! 1. 抽干窗口消息（键盘事件在这里翻译成 egui 事件）；
//! 2. 跑一帧 egui，拿到这一帧的形状；
//! 3. 把形状交给自写渲染器画进 DComp 交换链并呈现。

use eframe::egui::{Event, Key, Modifiers, RawInput, Rect};
use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE};

use crate::render::host::Host;
use crate::render::window::{take_keys, KeyEvent};

/// 虚拟键码 → egui 按键。只映射输入框真正用得上的。
fn to_egui_key(code: u32) -> Option<Key> {
    Some(match code {
        0x08 => Key::Backspace,
        0x09 => Key::Tab,
        0x0D => Key::Enter,
        0x1B => Key::Escape,
        0x20 => Key::Space,
        0x25 => Key::Home,
        0x26 => Key::ArrowUp,
        0x27 => Key::End,
        0x28 => Key::ArrowDown,
        0x2D => Key::Insert,
        0x2E => Key::Delete,
        0x21..=0x2E => Key::ArrowLeft,
        0x30..=0x39 => Key::Num0,
        0x41..=0x5A => Key::A,
        0x60..=0x69 => Key::Num0,
        0x70..=0x87 => Key::F1,
        _ => return None,
    })
}

/// 把收集到的键盘事件翻译成 egui 事件。
pub fn to_egui_events(events: &[KeyEvent]) -> Vec<Event> {
    let mut out = Vec::new();
    for event in events {
        match event {
            KeyEvent::Char(character) => out.push(Event::Text(character.to_string())),
            KeyEvent::Key(code, pressed) => {
                if let Some(key) = to_egui_key(*code) {
                    out.push(Event::Key {
                        key,
                        physical_key: None,
                        pressed: *pressed,
                        repeat: false,
                        modifiers: Modifiers::default(),
                    });
                }
            }
        }
    }
    out
}

/// 跑消息循环，直到 `frame` 返回 false。
///
/// `frame` 在 egui 的上下文里画这一帧；返回 false 表示退出。
pub fn run(
    host: &mut Host,
    screen_rect: Rect,
    mut frame: impl FnMut(&eframe::egui::Context) -> bool,
) -> Result<(), String> {
    let mut time = 0.0f64;
    loop {
        let events = to_egui_events(&take_keys());
        let raw = RawInput {
            screen_rect: Some(screen_rect),
            time: Some(time),
            events,
            focused: true,
            ..Default::default()
        };
        time += 1.0 / 60.0;
        let mut keep_going = true;
        let output = host.ctx.run(raw, |ctx| keep_going = frame(ctx));
        host.sync_fonts();
        let pixels_per_point = host.ctx.pixels_per_point();
        let primitives = host.ctx.tessellate(output.shapes, pixels_per_point);
        host.present(&primitives, pixels_per_point)?;
        if !keep_going {
            break;
        }
        pump_messages();
    }
    Ok(())
}

/// 抽干窗口消息。
pub fn pump_messages() {
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_become_text_events() {
        let events = to_egui_events(&[KeyEvent::Char('a'), KeyEvent::Char('灵')]);
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], Event::Text(text) if text == "a"));
        assert!(matches!(&events[1], Event::Text(text) if text == "灵"));
    }

    #[test]
    fn backspace_and_enter_become_key_events() {
        let events = to_egui_events(&[KeyEvent::Key(0x08, true), KeyEvent::Key(0x0D, true)]);
        assert_eq!(events.len(), 2);
        match &events[0] {
            Event::Key { key, pressed, .. } => {
                assert_eq!(*key, Key::Backspace);
                assert!(*pressed);
            }
            other => panic!("期望按键事件，得到 {other:?}"),
        }
    }

    #[test]
    fn unknown_keys_are_dropped() {
        assert!(to_egui_events(&[KeyEvent::Key(0x05, true)]).is_empty());
    }
}
