//! 自研后端的岛体窗口：自己建 HWND，收集键盘输入。
//!
//! 与 eframe 的窗口相比只保留必要的东西：
//!
//! - 岛体是显示层，鼠标交互由应用自己的全局光标跟踪负责（见 `native.rs`），
//!   所以窗口本身不吃鼠标消息；
//! - 键盘要真实送达（AI 输入框要能打字），因此把字符与常用按键收集起来，
//!   由消息循环翻译成 egui 的事件。

use std::sync::Mutex;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Foundation::{ERROR_CLASS_ALREADY_EXISTS, GetLastError, HINSTANCE};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

/// 窗口类名。
const CLASS_NAME: PCWSTR = w!("LingqiIslandWindow");

/// 一个待转发的键盘事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyEvent {
    /// 输入了一个字符（含中文输入法合成后的字符）。
    Char(char),
    /// 一个按键（虚拟键码 + 是否按下）。
    Key(u32, bool),
}

/// 键盘事件队列（窗口过程与消息循环之间传递）。
static KEYS: Mutex<Vec<KeyEvent>> = Mutex::new(Vec::new());

/// 取走并清空队列里的键盘事件。
pub fn take_keys() -> Vec<KeyEvent> {
    let mut keys = KEYS.lock().unwrap_or_else(|error| error.into_inner());
    std::mem::take(&mut *keys)
}

/// 岛体窗口。
pub struct IslandWindow {
    hwnd: HWND,
}

impl IslandWindow {
    /// 创建一个透明置顶的工具窗口。
    pub fn new(width: i32, height: i32, x: i32, y: i32) -> Result<IslandWindow, String> {
        unsafe {
            let instance = GetModuleHandleW(PCWSTR::null()).map_err(|error| format!("取不到模块句柄：{error}"))?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                hInstance: instance.into(),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            // 类可能已经注册过（第二次进入自研后端）。
            if RegisterClassW(&class) == 0 {
                let error = GetLastError();
                if error != ERROR_CLASS_ALREADY_EXISTS {
                    return Err(format!("注册窗口类失败：{error:?}"));
                }
            }
            let hwnd = CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                CLASS_NAME,
                w!("灵栖 Lingqi"),
                WS_POPUP,
                x,
                y,
                width,
                height,
                None,
                None,
                HINSTANCE(instance.0),
                None,
            )
            .map_err(|error| format!("创建岛体窗口失败：{error}"))?;
            Ok(IslandWindow { hwnd })
        }
    }

    /// 窗口句柄。
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// 移动并调整窗口大小。
    pub fn place(&self, x: i32, y: i32, width: i32, height: i32) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }
}

impl Drop for IslandWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// 窗口过程：只管键盘与绘制，其余交给系统。
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CHAR => {
            // 只收可打印字符（回车、退格等走 WM_KEYDOWN）。
            let code = (wparam.0 & 0xFFFF) as u32;
            if let Some(character) = char::from_u32(code) {
                if !character.is_control() {
                    let mut keys = KEYS.lock().unwrap_or_else(|error| error.into_inner());
                    keys.push(KeyEvent::Char(character));
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN | WM_KEYUP => {
            let code = (wparam.0 & 0xFFFF) as u32;
            let mut keys = KEYS.lock().unwrap_or_else(|error| error.into_inner());
            keys.push(KeyEvent::Key(code, message == WM_KEYDOWN));
            LRESULT(0)
        }
        WM_NCHITTEST => {
            // 岛体不吃鼠标：交互由应用自己的光标跟踪处理。
            LRESULT(HTTRANSPARENT as isize)
        }
        WM_PAINT => {
            let _ = DefWindowProcW(hwnd, message, wparam, lparam);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_queue_round_trips() {
        take_keys();
        KEYS.lock().unwrap().push(KeyEvent::Char('灵'));
        KEYS.lock().unwrap().push(KeyEvent::Key(0x08, true));
        let keys = take_keys();
        assert_eq!(keys, vec![KeyEvent::Char('灵'), KeyEvent::Key(0x08, true)]);
        assert!(take_keys().is_empty(), "取过之后队列应当是空的");
    }
}
