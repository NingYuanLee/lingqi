//! Windows 状态采样与经用户授权的通知读取。
use std::sync::mpsc::{self, Receiver};
use windows::core::{Result, HSTRING, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Threading::{GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetLastInputInfo, LASTINPUTINFO,
};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub volume: Option<(u32, bool)>,
    pub audio_endpoint: Option<String>,
    pub caps: bool,
    pub network: bool,
    pub network_kind: NetworkKind,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Observation {
    pub system: Snapshot,
    pub input: Option<crate::input::State>,
    pub fullscreen: Option<crate::display_layout::Bounds>,
}

/// 跨进程输入法查询和 COM 状态采样留在后台，只在状态改变时唤醒界面。
pub fn observe(ctx:eframe::egui::Context,initial:Snapshot,input:Option<crate::input::State>)->Receiver<Observation> {
    let(tx,rx)=mpsc::sync_channel(1);
    std::thread::Builder::new().name("lingqi-system".into()).spawn(move||{
        let mut current=Observation{system:initial,input,fullscreen:None};
        let mut delivered=None;
        let mut sampled=crate::poll_clock::PollClock::default();
        let mut input_at=sampled;
        loop {
            let identity=crate::input::State::identity();
            let changed=identity.map(|state|(state.context,state.layout))!=current.input.map(|state|(state.context,state.layout));
            if changed || crate::input::switch_recent() || input_at.ready(std::time::Duration::from_secs(1)){
                current.input=crate::input::State::read();input_at=crate::poll_clock::PollClock::now();
            }
            if sampled.ready(std::time::Duration::from_millis(500)){
                current.system=Snapshot::read();current.fullscreen=fullscreen();sampled=crate::poll_clock::PollClock::now();
            }
            if delivered.as_ref()!=Some(&current) {
                match tx.try_send(current.clone()) {
                    Ok(())=>{delivered=Some(current.clone());ctx.request_repaint();},
                    Err(mpsc::TrySendError::Disconnected(_))=>return,
                    Err(mpsc::TrySendError::Full(_))=>{},
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }).ok();
    rx
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetworkKind { Wifi, Ethernet, #[default] Other }
impl Snapshot {
    pub fn read() -> Self {
        unsafe {
            let device=audio_device().ok();
            let audio_endpoint=device.as_ref().and_then(|d|d.GetId().ok()).and_then(|id|{
                let text=id.to_string().ok();
                windows::Win32::System::Com::CoTaskMemFree(Some(id.0 as _));
                text
            });
            let volume = device.and_then(|d|d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL,None).ok()).and_then(|a| {
                Some((
                    (a.GetMasterVolumeLevelScalar().ok()? * 100.0).round() as u32,
                    a.GetMute().ok()?.as_bool(),
                ))
            });
            let profile=windows::Networking::Connectivity::NetworkInformation::GetInternetConnectionProfile().ok();
            let network=profile.as_ref().and_then(|p|p.GetNetworkConnectivityLevel().ok()).is_some_and(|v|v==windows::Networking::Connectivity::NetworkConnectivityLevel::InternetAccess);
            let network_kind=profile.as_ref().map(|p| {
                if p.IsWlanConnectionProfile().unwrap_or(false){NetworkKind::Wifi}
                else if p.NetworkAdapter().ok().and_then(|a|a.IanaInterfaceType().ok())==Some(6){NetworkKind::Ethernet}
                else{NetworkKind::Other}
            }).unwrap_or_default();
            Self{volume,audio_endpoint,network,network_kind,caps:GetKeyState(0x14)&1!=0}
        }
    }
    pub fn changes(&self, old: &Self) -> Vec<(String, String)> {
        let mut out = vec![];
        if self.audio_endpoint == old.audio_endpoint && self.volume != old.volume {
            if let Some((v, mute)) = self.volume {
                out.push((
                    "系统音量".into(),
                    if mute {
                        "已静音".into()
                    } else {
                        format!("音量 {v}%")
                    },
                ));
            }
        }
        if self.caps != old.caps {
            out.push((
                "大写锁定".into(),
                if self.caps { "已开启" } else { "已关闭" }.into(),
            ));
        }
        if self.network != old.network || self.network && self.network_kind != old.network_kind {
            let kind=if self.network {self.network_kind}else{old.network_kind};
            let status=if self.network {"网络已连接"}else{"网络已断开"};
            out.push((
                "网络连接".into(),
                match kind {NetworkKind::Wifi=>format!("Wi-Fi · {status}"),NetworkKind::Ethernet=>format!("以太网 · {status}"),NetworkKind::Other=>status.into()},
            ));
        }
        out
    }
}
unsafe fn audio_device() -> Result<IMMDevice> {    thread_local! {static INITIALIZED:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
    INITIALIZED.with(|v| {
        if !v.replace(true) {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
    });
    // 每个采样线程复用枚举器，但每次仍查询当前默认设备，保留热插拔响应。
    thread_local! {static ENUMERATOR:std::cell::RefCell<Option<IMMDeviceEnumerator>>=const{std::cell::RefCell::new(None)};}
    ENUMERATOR.with(|slot| {
        let mut slot=slot.borrow_mut();
        if slot.is_none(){*slot=Some(CoCreateInstance(&MMDeviceEnumerator,None,CLSCTX_ALL)?);}
        let result=slot.as_ref().unwrap().GetDefaultAudioEndpoint(eRender,eConsole);
        if result.is_err(){*slot=None;}
        result
    })
}
unsafe fn audio() -> Result<IAudioEndpointVolume> {audio_device()?.Activate(CLSCTX_ALL,None)}
/// 设置主音量（0..100）；同时解除静音，拖动音量条时立刻能听到。
#[cfg(windows)]
pub fn set_volume(percent: u32) {
    unsafe {
        if let Ok(a) = audio() {
            let _ = a.SetMasterVolumeLevelScalar((percent.min(100) as f32) / 100.0, std::ptr::null());
            let _ = a.SetMute(false, std::ptr::null());
        }
    }
}

/// 切换静音，返回切换后的静音状态（拿不到设备时返回 None）。
#[cfg(windows)]
pub fn toggle_mute() -> Option<bool> {
    unsafe {
        let a = audio().ok()?;
        let muted = a.GetMute().ok()?.as_bool();
        a.SetMute(!muted, std::ptr::null()).ok()?;
        Some(!muted)
    }
}

/// 当前会话是否锁屏。查询走 WTS（一次本机 RPC），按 2 秒缓存，避免每帧都查。
pub fn screen_locked() -> bool {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use windows::Win32::System::RemoteDesktop::{
        WTSFreeMemory, WTSQuerySessionInformationW, WTSSessionInfoEx, WTSINFOEXW, WTS_CURRENT_SERVER_HANDLE,
        WTS_CURRENT_SESSION, WTS_SESSIONSTATE_LOCK,
    };
    static CACHE: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, locked)) = *cache {
        if at.elapsed() < Duration::from_secs(2) { return locked; }
    }
    let locked = unsafe {
        let mut buf = windows::core::PWSTR::null();
        let mut len = 0u32;
        if WTSQuerySessionInformationW(WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTSSessionInfoEx, &mut buf, &mut len).is_ok() && !buf.is_null() {
            let info = &*(buf.0 as *const WTSINFOEXW);
            let locked = info.Level == 1 && info.Data.WTSInfoExLevel1.SessionFlags == WTS_SESSIONSTATE_LOCK as i32;
            WTSFreeMemory(buf.0 as *mut core::ffi::c_void);
            locked
        } else {
            false
        }
    };
    *cache = Some((Instant::now(), locked));
    locked
}

pub fn idle_seconds() -> f64 {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            ..Default::default()
        };
        if GetLastInputInfo(&mut info).as_bool() {
            windows::Win32::System::SystemInformation::GetTickCount().wrapping_sub(info.dwTime)
                as f64
                / 1000.0
        } else {
            0.0
        }
    }
}
/// 前台窗口在全屏判定里需要的信息。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Foreground {
    hwnd: isize,
    pid: u32,
    /// 最终所有者窗口（没有所有者时是它自己）。
    root_owner: isize,
    /// 会在任务栏单独显示按钮的普通程序窗口（不是工具窗口、对话框或浮动面板）。
    app_window: bool,
}

/// 前台从全屏窗口切到别的窗口时，是否仍按原来的全屏处理（灵栖保持隐藏）。
///
/// 3ds Max、Photoshop 这类软件全屏时，插件面板、浮动工具条、
/// 属性对话框都是同一个程序的附属窗口；每点一下前台就在它们和主窗口之间来回切，
/// 过去灵栖会跟着「出现 → 消失」闪一次。只要原来的全屏窗口还盖着屏幕，
/// 而新的前台是它的附属窗口（被它拥有，或同一程序里不单独占任务栏的窗口），就继续隐藏。
/// 切到另一个程序、或同一程序里另开的普通窗口（如浏览器的新窗口）时照常显示。
fn keeps_fullscreen(last: Foreground, still_covers: bool, now: Option<Foreground>) -> bool {
    if !still_covers { return false; }
    match now {
        // 激活切换的瞬间前台可能是空的，不当成离开全屏。
        None => true,
        Some(now) => now.hwnd == last.hwnd || now.root_owner == last.hwnd || (now.pid == last.pid && !now.app_window),
    }
}

unsafe fn foreground_info(hwnd: HWND) -> Foreground {
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    let root = GetAncestor(hwnd, GA_ROOTOWNER);
    let root_owner = if root.0.is_null() { hwnd.0 as isize } else { root.0 as isize };
    let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    let owned = !GetWindow(hwnd, GW_OWNER).unwrap_or_default().0.is_null();
    let app_window = ex & WS_EX_APPWINDOW.0 as isize != 0 || (!owned && ex & WS_EX_TOOLWINDOW.0 as isize == 0);
    Foreground { hwnd: hwnd.0 as isize, pid, root_owner, app_window }
}

/// 窗口盖满它所在的整块屏幕时返回该屏幕范围。
unsafe fn covering_monitor(hwnd: HWND) -> Option<crate::display_layout::Bounds> {
    if !IsWindow(hwnd).as_bool() || !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
        return None;
    }
    let mut r = RECT::default();
    if GetWindowRect(hwnd, &mut r).is_err() {
        return None;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info).as_bool()
    {
        return None;
    }
    let covers=r.left <= info.rcMonitor.left
        && r.top <= info.rcMonitor.top
        && r.right >= info.rcMonitor.right
        && r.bottom >= info.rcMonitor.bottom;
    // 任务栏自动隐藏时，普通最大化窗口的外框也可能覆盖整屏，但标题栏占掉了客户区顶部。
    // 以客户区是否盖满整屏为准，不看样式位：有的游戏全屏时保留标题栏样式和最大化标志、
    // 实际并不画标题栏，按样式位判断会把它们当成普通最大化（0.6.65 的无畏契约回归）。
    if covers && IsZoomed(hwnd).as_bool() && !client_covers(hwnd, &info.rcMonitor) {
        #[cfg(not(test))] log_maximized_exclusion(hwnd, r, info.rcMonitor);
        return None;
    }
    covers.then_some(crate::display_layout::Bounds {left:info.rcMonitor.left,top:info.rcMonitor.top,right:info.rcMonitor.right,bottom:info.rcMonitor.bottom})
}

/// 客户区（不含标题栏和边框）是否盖满整块屏幕。
unsafe fn client_covers(hwnd: HWND, monitor: &RECT) -> bool {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    let mut c = RECT::default();
    if GetClientRect(hwnd, &mut c).is_err() { return false; }
    let (mut tl, mut br) = (POINT { x: c.left, y: c.top }, POINT { x: c.right, y: c.bottom });
    if !ClientToScreen(hwnd, &mut tl).as_bool() || !ClientToScreen(hwnd, &mut br).as_bool() { return false; }
    tl.x <= monitor.left && tl.y <= monitor.top && br.x >= monitor.right && br.y >= monitor.bottom
}

/// 诊断：盖满屏幕却按普通最大化处理（不隐藏灵栖）的窗口，每个窗口只记一次，便于核对用户反馈。
#[cfg(not(test))]
unsafe fn log_maximized_exclusion(hwnd: HWND, window: RECT, monitor: RECT) {
    use std::sync::Mutex;
    static LOGGED: Mutex<isize> = Mutex::new(0);
    let mut logged = LOGGED.lock().unwrap_or_else(|e| e.into_inner());
    if *logged == hwnd.0 as isize { return; }
    *logged = hwnd.0 as isize;
    let mut cls = [0u16; 128];
    let n = GetClassNameW(hwnd, &mut cls);
    let mut c = RECT::default();
    let _ = GetClientRect(hwnd, &mut c);
    let line = format!("{} 普通最大化不隐藏：类名={} 样式=0x{:08x} 扩展=0x{:08x} 窗口={:?} 客户区={}×{} 屏幕={:?}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), String::from_utf16_lossy(&cls[..n.max(0) as usize]),
        GetWindowLongPtrW(hwnd, GWL_STYLE) as u32, GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32,
        (window.left, window.top, window.right, window.bottom), c.right - c.left, c.bottom - c.top,
        (monitor.left, monitor.top, monitor.right, monitor.bottom));
    crate::resources::append_log(&crate::model::config_dir().join("diagnostics"), "fullscreen.log", &line);
}

pub fn fullscreen() -> Option<crate::display_layout::Bounds> {
    use std::sync::Mutex;
    // 上一次判定为全屏的窗口：前台切到它的附属窗口时沿用（见 `keeps_fullscreen`）。
    static LAST: Mutex<Option<Foreground>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        let hwnd = GetForegroundWindow();
        let now = (!hwnd.0.is_null()).then(|| foreground_info(hwnd));
        if let Some(info) = now {
            // 灵栖自己的窗口（设置窗口等）不算全屏，也不沿用。
            if info.pid == windows::Win32::System::Threading::GetCurrentProcessId() {
                *last = None;
                return None;
            }
            // 桌面与资源管理器桌面窗口不能当成全屏游戏。
            let mut cls = [0u16; 64];
            let n = GetClassNameW(hwnd, &mut cls);
            let cls = String::from_utf16_lossy(&cls[..n.max(0) as usize]);
            if matches!(cls.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd") {
                *last = None;
                return None;
            }
            if let Some(bounds) = covering_monitor(hwnd) {
                *last = Some(info);
                return Some(bounds);
            }
        }
        if let Some(previous) = *last {
            let bounds = covering_monitor(HWND(previous.hwnd as *mut _));
            if keeps_fullscreen(previous, bounds.is_some(), now) {
                return bounds;
            }
        }
        *last = None;
        None
    }
}

#[cfg(test)]
mod fullscreen_follow_tests {
    use super::*;

    const MAX: Foreground = Foreground { hwnd: 100, pid: 7, root_owner: 100, app_window: true };

    #[test]
    fn maximized_browser_and_borderless_fullscreen_at_the_same_bounds_are_distinct() {
        unsafe {
            let hwnd = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW, windows::core::w!("STATIC"),
                windows::core::w!(""), WS_OVERLAPPEDWINDOW | WS_MAXIMIZE, 0, 0, 320, 240, None, None, None, None).unwrap();
            let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            assert!(GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info).as_bool());
            let r = info.rcMonitor;
            let cover = || SetWindowPos(hwnd, HWND_BOTTOM, r.left, r.top, r.right-r.left, r.bottom-r.top,
                SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED).unwrap();
            cover();
            assert!(IsZoomed(hwnd).as_bool());
            let maximized = covering_monitor(hwnd);
            // Chromium 进入 F11 时移除标题栏和可调整大小的边框，仍可能保留最大化标志。
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            SetWindowLongPtrW(hwnd, GWL_STYLE, style & !((WS_CAPTION | WS_THICKFRAME).0 as isize));
            cover();
            let fullscreen = covering_monitor(hwnd);
            DestroyWindow(hwnd).unwrap();
            assert!(fullscreen.is_some(), "无边框全屏仍应隐藏灵栖");
            assert!(maximized.is_none(), "普通最大化即使盖满屏幕也应显示灵栖");
        }
    }

    #[test]
    fn game_that_keeps_caption_style_but_draws_no_caption_still_hides_the_island() {
        use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
        unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
            // 不留非客户区：样式里有标题栏，但客户区就是整个窗口（自绘或无边框全屏的游戏）。
            if m == WM_NCCALCSIZE && w.0 != 0 { return LRESULT(0); }
            DefWindowProcW(h, m, w, l)
        }
        unsafe {
            let class = windows::core::w!("LingqiCaptionlessGame");
            let wc = WNDCLASSW { lpfnWndProc: Some(proc), lpszClassName: class, ..Default::default() };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW, class, windows::core::w!(""),
                WS_OVERLAPPEDWINDOW | WS_MAXIMIZE, 0, 0, 320, 240, None, None, None, None).unwrap();
            let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            assert!(GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info).as_bool());
            let r = info.rcMonitor;
            SetWindowPos(hwnd, HWND_BOTTOM, r.left, r.top, r.right-r.left, r.bottom-r.top,
                SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED).unwrap();
            let zoomed = IsZoomed(hwnd).as_bool();
            let captioned = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_CAPTION.0 == WS_CAPTION.0;
            let result = covering_monitor(hwnd);
            DestroyWindow(hwnd).unwrap();
            assert!(zoomed && captioned, "复现条件：最大化且保留标题栏样式");
            assert!(result.is_some(), "客户区盖满整屏的全屏游戏应隐藏灵栖");
        }
    }

    #[test]
    fn plugin_panels_and_dialogs_of_the_fullscreen_app_keep_the_island_hidden() {
        // 被主窗口拥有的插件面板 / 对话框
        let owned = Foreground { hwnd: 200, pid: 7, root_owner: 100, app_window: false };
        assert!(keeps_fullscreen(MAX, true, Some(owned)));
        // 同一程序里不占任务栏的浮动工具窗口（没有设所有者）
        let tool = Foreground { hwnd: 201, pid: 7, root_owner: 201, app_window: false };
        assert!(keeps_fullscreen(MAX, true, Some(tool)));
        // 插件放在独立进程里、但窗口挂在主窗口下面
        let other_process = Foreground { hwnd: 202, pid: 9, root_owner: 100, app_window: false };
        assert!(keeps_fullscreen(MAX, true, Some(other_process)));
        // 激活切换瞬间前台为空
        assert!(keeps_fullscreen(MAX, true, None));
    }

    #[test]
    fn switching_to_another_app_or_leaving_fullscreen_shows_the_island() {
        let other_app = Foreground { hwnd: 300, pid: 9, root_owner: 300, app_window: true };
        assert!(!keeps_fullscreen(MAX, true, Some(other_app)), "切到别的程序时照常显示");
        // 同一程序新开的普通窗口（如浏览器的第二个窗口）也照常显示
        let second_window = Foreground { hwnd: 301, pid: 7, root_owner: 301, app_window: true };
        assert!(!keeps_fullscreen(MAX, true, Some(second_window)));
        // 原来的全屏窗口已经还原、最小化或关闭
        let owned = Foreground { hwnd: 200, pid: 7, root_owner: 100, app_window: false };
        assert!(!keeps_fullscreen(MAX, false, Some(owned)));
        assert!(!keeps_fullscreen(MAX, false, None));
    }
}

pub(crate) fn is_browser_process(name:&str)->bool {
    ["chrome.exe","msedge.exe","firefox.exe","brave.exe","vivaldi.exe","opera.exe",
        "arc.exe","zen.exe","chromium.exe","librewolf.exe","floorp.exe","waterfox.exe",
        "duckduckgo.exe","360se.exe","360chrome.exe","qqbrowser.exe","sogouexplorer.exe","quark.exe"]
        .iter().any(|browser|name.eq_ignore_ascii_case(browser))
}

fn browser_overlaps_island(browser:RECT,island:RECT,dock:crate::model::Dock)->bool {
    if dock.is_side() {return window_covers_side(browser,island,dock);}
    // 窗口是 CANVAS_W 宽的透明画布，岛体中心固定在画布左侧 WIN_W/2 处（与水平位置、大小无关）。
    let scale=(island.right-island.left) as f32/crate::app::CANVAS_W;
    let center=island.left as f32+crate::app::WIN_W*scale/2.0;
    let half=crate::app::PW*scale/2.0;
    (browser.left as f32)<center+half && (browser.right as f32)>center-half
        && if dock==crate::model::Dock::Bottom {
            browser.top<island.bottom && (browser.bottom as f32)>island.bottom as f32-crate::app::BAR_H*scale
        }else{(browser.top as f32)<island.top as f32+crate::app::BAR_H*scale && browser.bottom>island.top}
}

/// 左右停靠：画布是 SIDE_W × SIDE_H，岛体贴在画布的左 / 右边、竖直居中。
/// 前台窗口盖住岛所在的那一段屏幕边缘（例如最大化、全屏或贴边摆放）即视为遮挡。
fn window_covers_side(window:RECT,island:RECT,dock:crate::model::Dock)->bool {
    let scale=(island.right-island.left) as f32/crate::side_dock::SIDE_W;
    let cy=island.top as f32+crate::side_dock::SIDE_H*scale/2.0;
    let half=crate::app::PH*scale/2.0;
    let strip=crate::side_dock::SIDE_BAR_W*scale;
    let vertical=(window.top as f32)<cy+half && (window.bottom as f32)>cy-half;
    let horizontal=if dock==crate::model::Dock::Left {(window.left as f32)<island.left as f32+strip && window.right>island.left}
        else {(window.right as f32)>island.right as f32-strip && window.left<island.right};
    vertical && horizontal
}

pub fn browser_at_top(bounds:RECT,settings:&crate::model::Settings)->Option<bool> {
    unsafe {
        let foreground=GetForegroundWindow();
        if foreground.0.is_null(){return Some(false);}
        let mut pid=0;
        GetWindowThreadProcessId(foreground,Some(&mut pid));
        // 设置、托盘和岛内输入沿用此前的避让状态，避免自身窗口引起反复切换。
        if pid==GetCurrentProcessId(){return None;}
        if pid==0{return Some(false);}
        thread_local! { static LAST_MAIN: std::cell::Cell<Option<(HWND,u32)>> = const { std::cell::Cell::new(None) }; }
        LAST_MAIN.with(|last| avoidance_for_window(foreground,bounds,pid,settings,last,||process_image_path(pid)))
    }
}

unsafe fn visible_avoidance_rect(hwnd:HWND)->Option<RECT> {
    if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool(){return None;}
    let mut cloaked=0u32;
    let _=windows::Win32::Graphics::Dwm::DwmGetWindowAttribute(hwnd,windows::Win32::Graphics::Dwm::DWMWA_CLOAKED,(&mut cloaked as *mut u32).cast(),4);
    if cloaked!=0{return None;}
    let mut rect=RECT::default();
    GetWindowRect(hwnd,&mut rect).ok()?;
    Some(rect)
}

unsafe fn process_image_path(pid:u32)->Option<String> {
    let process=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,false,pid).ok()?;
    let mut path=vec![0u16;32768];let mut length=path.len() as u32;
    let result=QueryFullProcessImageNameW(process,PROCESS_NAME_WIN32,PWSTR(path.as_mut_ptr()),&mut length);
    let _=CloseHandle(process);
    result.ok()?;
    Some(String::from_utf16_lossy(&path[..length as usize]))
}

// 无所有者的工具窗可能在灵栖启动时就已打开。只有同进程的主窗口唯一时才补认宿主。
unsafe fn unique_main_window(pid:u32)->Option<HWND> {
    use windows::Win32::Foundation::{BOOL,LPARAM};
    struct Search { pid:u32, main:Option<HWND> }
    unsafe extern "system" fn visit(hwnd:HWND,param:LPARAM)->BOOL {
        let search=&mut *(param.0 as *mut Search);
        let mut pid=0;GetWindowThreadProcessId(hwnd,Some(&mut pid));
        if pid==search.pid && foreground_info(hwnd).app_window && visible_avoidance_rect(hwnd).is_some() {
            if search.main.is_some(){search.main=None;return BOOL(0);}
            search.main=Some(hwnd);
        }
        BOOL(1)
    }
    let mut search=Search{pid,main:None};
    EnumWindows(Some(visit),LPARAM((&mut search as *mut Search) as isize)).ok()?;
    search.main
}

unsafe fn process_covers_island(pid:u32,island:RECT,dock:crate::model::Dock)->bool {
    use windows::Win32::Foundation::{BOOL,LPARAM};
    struct Search { pid:u32, island:RECT, dock:crate::model::Dock, covered:bool }
    unsafe extern "system" fn visit(hwnd:HWND,param:LPARAM)->BOOL {
        let search=&mut *(param.0 as *mut Search);
        let mut pid=0;GetWindowThreadProcessId(hwnd,Some(&mut pid));
        if pid==search.pid && visible_avoidance_rect(hwnd).is_some_and(|rect|browser_overlaps_island(rect,search.island,search.dock)) {
            search.covered=true;
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut search=Search{pid,island,dock,covered:false};
    let _=EnumWindows(Some(visit),LPARAM((&mut search as *mut Search) as isize));
    search.covered
}

unsafe fn avoidance_for_window(foreground:HWND,island:RECT,pid:u32,settings:&crate::model::Settings,last:&std::cell::Cell<Option<(HWND,u32)>>,image_path:impl FnOnce()->Option<String>)->Option<bool> {
    if !settings.browser_minimal || IsIconic(GetAncestor(foreground,GA_ROOTOWNER)).as_bool(){return Some(false);}
    let rect=visible_avoidance_rect(foreground);
    let overlaps=|rect|browser_overlaps_island(rect,island,settings.dock);
    let owner=GetWindow(foreground,GW_OWNER).ok();
    let info=foreground_info(foreground);
    if rect.is_some() && owner.is_none() && info.app_window {last.set(Some((foreground,pid)));}
    else if rect.is_some() && last.get().is_some_and(|(_,prev)|prev!=pid){last.set(None);}
    let path=std::cell::LazyCell::new(image_path);
    let matches=|p:&str|avoidance_matches(p,settings);
    if rect.is_some_and(overlaps) && path.as_deref().is_some_and(matches){return Some(true);}
    if !settings.avoid_browsers && settings.avoid_apps.is_empty(){return Some(false);}

    // 3ds Max 的插件可能以独立窗口获得焦点，或短暂无法读取可见矩形。
    // 只在已登记的 Max 进程仍有窗口实际遮岛时延续避让。
    if !settings.avoid_apps.is_empty() && path.as_deref().is_some_and(|p| {
        std::path::Path::new(p).file_name().is_some_and(|name|name.eq_ignore_ascii_case("3dsmax.exe"))
            && crate::avoidance::contains_app(&settings.avoid_apps,p)
    }) && process_covers_island(pid,island,settings.dock) {return Some(true);}

    // 浮窗暂时读不到矩形时，仍可沿所有者链找到盖住岛体的窗口。
    // 这条关系也适用于由其他进程创建的浮窗。
    let mut parent=owner;
    while let Some(hwnd)=parent {
        if IsIconic(hwnd).as_bool(){return Some(false);}
        if visible_avoidance_rect(hwnd).is_some_and(overlaps) {
            let mut owner_pid=0;GetWindowThreadProcessId(hwnd,Some(&mut owner_pid));
            let registered=if owner_pid==pid {path.as_deref().is_some_and(matches)}
                else {process_image_path(owner_pid).as_deref().is_some_and(matches)};
            if registered{
                if foreground_info(hwnd).app_window {last.set(Some((hwnd,owner_pid)));}
                return Some(true);
            }
        }
        parent=GetWindow(hwnd,GW_OWNER).ok();
    }
    if owner.is_some() || info.app_window || !path.as_deref().is_some_and(matches){return Some(false);}
    // 部分插件工具窗未登记 owner。优先跟随刚才操作的同进程主窗口，每次重新检查其位置。
    let host=match last.get() {
        Some((hwnd,prev)) if prev==pid && foreground_info(hwnd).pid==pid=>Some(hwnd),
        _=>unique_main_window(pid),
    };
    Some(host.and_then(|hwnd|visible_avoidance_rect(hwnd)).is_some_and(overlaps))
}

pub fn pick_files() -> Result<Vec<std::path::PathBuf>> {
    pick_files_for("添加文件到置物架",true)
}

pub fn pick_files_for(title:&str,multiple:bool) -> Result<Vec<std::path::PathBuf>> {
    use windows::Win32::UI::Shell::*;
    let mut options=FOS_FILEMUSTEXIST;
    if multiple{options|=FOS_ALLOWMULTISELECT;}
    pick_items_for(title,options)
}

pub fn pick_folder_for(title:&str) -> Result<Option<std::path::PathBuf>> {
    use windows::Win32::UI::Shell::*;
    Ok(pick_items_for(title,FOS_PICKFOLDERS | FOS_PATHMUSTEXIST)?.into_iter().next())
}

fn pick_items_for(title:&str,options:windows::Win32::UI::Shell::FILEOPENDIALOGOPTIONS) -> Result<Vec<std::path::PathBuf>> {
    unsafe {
        use windows::Win32::UI::Shell::*;
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)?;
        dialog.SetOptions(FOS_FORCEFILESYSTEM | options)?;
        dialog.SetTitle(&HSTRING::from(title))?;
        if let Err(e) = dialog.Show(None) {
            if e.code().0 as u32 == 0x800704c7 {
                return Ok(vec![]);
            }
            return Err(e);
        }
        let items = dialog.GetResults()?;
        let mut out = vec![];
        for i in 0..items.GetCount()? {
            let ptr = items.GetItemAt(i)?.GetDisplayName(SIGDN_FILESYSPATH)?;
            let text = ptr.to_string()?;
            windows::Win32::System::Com::CoTaskMemFree(Some(ptr.0 as _));
            out.push(text.into());
        }
        Ok(out)
    }
}
/// 构建资源管理器可识别的文件数据对象。
fn file_data_object(path: &std::path::Path) -> Result<windows::Win32::System::Com::IDataObject> {
    unsafe {
        use windows::Win32::UI::Shell::*;
        let wide: Vec<u16> = path
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let pidl = ILCreateFromPathW(windows::core::PCWSTR(wide.as_ptr()));
        if pidl.is_null() {
            return Err(windows::core::Error::from_win32());
        }
        let result = (|| {
            let mut child = std::ptr::null_mut();
            let folder: IShellFolder = SHBindToParent(pidl, Some(&mut child))?;
            folder.GetUIObjectOf(None, &[child as *const _], None)
        })();
        ILFree(Some(pidl));
        result
    }
}
/// 原生拖出只允许复制，取消拖放不会改变置物架与原文件。
pub fn drag_file(path: &std::path::Path) -> Result<()> {
    unsafe {
        use windows::Win32::System::Ole::{OleInitialize, OleUninitialize, DROPEFFECT_COPY};
        use windows::Win32::UI::Shell::SHDoDragDrop;
        let initialized = OleInitialize(None).is_ok();
        let result = file_data_object(path)
            .and_then(|data| SHDoDragDrop(None, &data, None, DROPEFFECT_COPY).map(|_| ()));
        if initialized {
            OleUninitialize();
        }
        result
    }
}

fn avoidance_matches(path:&str,settings:&crate::model::Settings)->bool {
    settings.browser_minimal && (crate::avoidance::contains_app(&settings.avoid_apps,path)
        || settings.avoid_browsers && std::path::Path::new(path).file_name().and_then(|name|name.to_str()).is_some_and(is_browser_process))
}

pub(crate) fn avoidance_windows()->std::result::Result<Vec<crate::avoidance::Candidate>,String> {
    use windows::Win32::Foundation::{BOOL,LPARAM};
    unsafe extern "system" fn visit(hwnd:HWND,param:LPARAM)->BOOL {
        if !IsWindowVisible(hwnd).as_bool() || GetWindowLongPtrW(hwnd,GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0!=0 {return BOOL(1);}
        let mut cloaked=0u32;
        let _=windows::Win32::Graphics::Dwm::DwmGetWindowAttribute(hwnd,windows::Win32::Graphics::Dwm::DWMWA_CLOAKED,(&mut cloaked as *mut u32).cast(),4);
        if cloaked!=0{return BOOL(1);}
        let mut title=[0u16;1024];let len=GetWindowTextW(hwnd,&mut title);
        if len==0{return BOOL(1);}
        let mut pid=0;GetWindowThreadProcessId(hwnd,Some(&mut pid));
        if pid==GetCurrentProcessId(){return BOOL(1);}
        let Some(path)=process_image_path(pid) else{return BOOL(1)};
        let items=&mut *(param.0 as *mut Vec<crate::avoidance::Candidate>);
        if items.iter().any(|a|crate::avoidance::same_app(&a.path,&path)){return BOOL(1);}
        let filename=std::path::Path::new(&path).file_name().and_then(|s|s.to_str()).unwrap_or("");
        let name=executable_description(&path).unwrap_or_else(||filename.trim_end_matches(".exe").to_string());
        items.push(crate::avoidance::Candidate{browser:is_browser_process(filename),path,name,title:String::from_utf16_lossy(&title[..len as usize])});
        BOOL(1)
    }
    let mut items:Vec<crate::avoidance::Candidate>=Vec::new();
    unsafe{EnumWindows(Some(visit),LPARAM(&mut items as *mut _ as isize))}.map_err(|_|"无法读取应用列表，请刷新重试".to_string())?;
    items.sort_by_key(|a|a.name.to_lowercase());
    Ok(items)
}

fn executable_description(path:&str)->Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW,GetFileVersionInfoW,VerQueryValueW};
    let path:Vec<u16>=path.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let size=GetFileVersionInfoSizeW(PCWSTR(path.as_ptr()),None);if size==0{return None;}
        let mut bytes=vec![0u8;size as usize];GetFileVersionInfoW(PCWSTR(path.as_ptr()),0,size,bytes.as_mut_ptr().cast()).ok()?;
        let mut ptr=std::ptr::null_mut();let mut len=0;
        if !VerQueryValueW(bytes.as_ptr().cast(),windows::core::w!("\\VarFileInfo\\Translation"),&mut ptr,&mut len).as_bool() || len<4{return None;}
        let translation=std::slice::from_raw_parts(ptr as *const u16,2);
        let key:Vec<u16>=format!("\\StringFileInfo\\{:04x}{:04x}\\FileDescription",translation[0],translation[1]).encode_utf16().chain(Some(0)).collect();
        if !VerQueryValueW(bytes.as_ptr().cast(),PCWSTR(key.as_ptr()),&mut ptr,&mut len).as_bool() || len<=1{return None;}
        let name=String::from_utf16_lossy(std::slice::from_raw_parts(ptr as *const u16,len as usize)).trim_end_matches('\0').trim().to_string();
        (!name.is_empty()).then_some(name)
    }
}

#[cfg(test)]
mod avoidance_tests {
    use super::*;
    #[test]
    fn registered_3ds_max_uses_visible_same_process_windows_for_avoidance() {
        unsafe {
            let create=|x,y,width,height| CreateWindowExW(WS_EX_NOACTIVATE|WS_EX_APPWINDOW,windows::core::w!("STATIC"),
                windows::core::w!(""),WS_POPUP|WS_VISIBLE,x-10000,y-10000,width,height,None,None,None,None).unwrap();
            let island=RECT{left:-10000,top:-10000,right:-9040,bottom:-9900};
            let host=create(0,0,1920,1080);
            let panel=create(600,300,300,200);
            let mut settings=crate::model::Settings::default();
            settings.browser_minimal=true;
            let path=r"D:\Apps\Autodesk\3DSMAX.EXE";
            settings.avoid_apps.push((path.into(),"3ds Max".into()));
            let last=std::cell::Cell::new(None);
            let poll=|path:&str| {
                avoidance_for_window(panel,island,GetCurrentProcessId(),&settings,&last,||Some(path.into()))
            };
            let detached=poll(path);
            let unregistered=poll(r"D:\Other\3dsmax.exe");
            let _=ShowWindow(panel,SW_HIDE);
            let hidden_panel=poll(path);
            let _=ShowWindow(panel,SW_SHOWNOACTIVATE);
            SetWindowPos(host,HWND_BOTTOM,-8000,-10000,1920,1080,SWP_NOACTIVATE).unwrap();
            let moved=poll(path);
            SetWindowPos(host,HWND_BOTTOM,-10000,-10000,1920,1080,SWP_NOACTIVATE).unwrap();
            let _=ShowWindow(host,SW_SHOWMINNOACTIVE);
            let minimized=poll(path);
            DestroyWindow(panel).unwrap();
            DestroyWindow(host).unwrap();
            assert_eq!(detached,Some(true),"独立浮窗获得焦点时，仍遮住岛的 Max 窗口应触发避让");
            assert_eq!(hidden_panel,Some(true),"前台浮窗短暂不可用时，仍应检查实际遮住岛的窗口");
            assert_eq!([unregistered,moved,minimized],[Some(false);3]);
        }
    }

    #[test]
    fn registered_app_keeps_avoidance_when_its_plugin_panel_takes_focus() {
        unsafe {
            let create=|x,y,width,height,owner,ex| CreateWindowExW(WS_EX_NOACTIVATE|ex,windows::core::w!("STATIC"),
                windows::core::w!(""),WS_POPUP|WS_VISIBLE,x-10000,y-10000,width,height,owner,None,None,None).unwrap();
            let island=create(0,0,960,100,None,WS_EX_TOOLWINDOW);
            let host=create(0,0,1920,1080,None,WS_EX_APPWINDOW);
            let panel=create(600,300,300,200,Some(&host),WS_EX_TOOLWINDOW);
            let path=std::env::current_exe().unwrap();
            let mut settings=crate::model::Settings::default();
            settings.browser_minimal=true;
            settings.avoid_apps.push((path.to_string_lossy().into_owned(),"测试宿主".into()));
            let last=std::cell::Cell::new(None);
            let mut island_rect=RECT::default();
            GetWindowRect(island,&mut island_rect).unwrap();
            let poll=|window,settings:&crate::model::Settings| {
                let pid=GetCurrentProcessId();
                avoidance_for_window(window,island_rect,pid,settings,&last,||process_image_path(pid))
            };
            let results=[poll(host,&settings),poll(panel,&settings),poll(host,&settings)];
            let detached=create(600,300,300,200,None,WS_EX_TOOLWINDOW);
            let nested=create(700,400,100,100,Some(&panel),WS_EX_TOOLWINDOW);
            assert_eq!(poll(detached,&settings),Some(true),"无 owner 的同进程工具窗沿用刚获得焦点的主窗口");
            assert_eq!(poll(nested,&settings),Some(true),"嵌套插件对话框应找到宿主");
            assert_eq!(avoidance_for_window(panel,island_rect,0,&settings,&last,||None),Some(true),"面板进程不可识别时仍可通过所有者识别宿主");
            let independent=create(600,300,300,200,None,WS_EX_APPWINDOW);
            last.set(None);
            assert_eq!(poll(detached,&settings),Some(false),"两个可见主窗且无法确定宿主时，不借用后台窗口");
            let small=poll(independent,&settings);
            assert_eq!(poll(detached,&settings),Some(false),"当前主窗不遮岛时不能借用同进程另一个大窗");
            SetWindowPos(independent,HWND_BOTTOM,-9700,-9995,96,80,SWP_NOACTIVATE).unwrap();
            assert_eq!(poll(independent,&settings),Some(true),"小窗确实遮住岛时也要避让");
            assert_eq!(poll(host,&settings),Some(true));

            SetWindowPos(host,HWND_BOTTOM,-8000,-10000,1920,1080,SWP_NOACTIVATE).unwrap();
            let other_screen=[poll(host,&settings),poll(panel,&settings)];
            SetWindowPos(panel,HWND_TOP,-9700,-9995,96,80,SWP_NOACTIVATE).unwrap();
            assert_eq!(poll(panel,&settings),Some(true),"宿主在副屏时，浮窗自身遮住岛也应避让");
            SetWindowPos(panel,HWND_TOP,-9400,-9700,300,200,SWP_NOACTIVATE).unwrap();
            SetWindowPos(host,HWND_BOTTOM,-10000,-10000,1920,1080,SWP_NOACTIVATE).unwrap();
            let _=ShowWindow(host,SW_SHOWMINNOACTIVE);
            assert!(IsIconic(host).as_bool());
            let minimized=poll(host,&settings);
            let minimized_owner=poll(panel,&settings);
            let _=ShowWindow(host,SW_SHOWNOACTIVATE);
            let _=ShowWindow(panel,SW_HIDE);
            let hidden_panel=poll(panel,&settings);
            let hidden_panel_with_unknown_process=avoidance_for_window(panel,island_rect,0,&settings,&last,||None);
            let _=ShowWindow(detached,SW_HIDE);
            let hidden_tool=poll(detached,&settings);
            let _=ShowWindow(independent,SW_HIDE);
            let hidden_independent=poll(independent,&settings);
            let _=ShowWindow(host,SW_HIDE);
            let hidden_host=poll(panel,&settings);
            let _=ShowWindow(host,SW_SHOWNOACTIVATE);
            let _=ShowWindow(panel,SW_SHOWNOACTIVATE);
            settings.browser_minimal=false;
            let disabled=poll(panel,&settings);
            settings.browser_minimal=true;
            settings.avoid_apps.clear();
            let removed=poll(panel,&settings);
            DestroyWindow(detached).unwrap();
            DestroyWindow(independent).unwrap();
            DestroyWindow(host).unwrap();
            DestroyWindow(island).unwrap();
            assert_eq!(results,[Some(true);3],"焦点从宿主切到中央插件浮窗再切回时应持续避让");
            assert_eq!(small,Some(false),"同进程的独立小窗只检查自己的位置");
            assert_eq!(other_screen,[Some(false);2],"主窗口移到副屏后，浮窗也应停止避让");
            assert_eq!([hidden_panel,hidden_panel_with_unknown_process,hidden_tool],[Some(true);3],"前台浮窗不可见时仍应检查已识别的宿主");
            assert_eq!([minimized,minimized_owner,hidden_host,hidden_independent],[Some(false);4],"宿主收起或隐藏时结束避让，独立小窗也不能借用后台窗口");
            assert_eq!((disabled,removed),(Some(false),Some(false)));
        }
    }

    #[test]
    fn browser_popup_follows_its_owner_when_browser_avoidance_is_enabled() {
        unsafe {
            let create=|x,y,width,height,owner| CreateWindowExW(WS_EX_NOACTIVATE,windows::core::w!("STATIC"),
                windows::core::w!(""),WS_POPUP|WS_VISIBLE,x-10000,y-10000,width,height,owner,None,None,None).unwrap();
            let island=RECT{left:-10000,top:-10000,right:-9040,bottom:-9900};
            let host=create(0,0,1920,1080,None);
            let panel=create(600,300,300,200,Some(&host));
            let _=ShowWindow(panel,SW_HIDE);
            let mut settings=crate::model::Settings::default();
            settings.browser_minimal=true;
            let last=std::cell::Cell::new(None);
            let poll=|settings:&crate::model::Settings| {
                avoidance_for_window(panel,island,GetCurrentProcessId(),settings,&last,||Some(r"D:\Apps\chrome.exe".into()))
            };
            let enabled=poll(&settings);
            settings.avoid_browsers=false;
            let disabled=poll(&settings);
            DestroyWindow(host).unwrap();
            assert_eq!((enabled,disabled),(Some(true),Some(false)));
        }
    }

    #[test]
    fn avoidance_respects_master_browser_switch_and_custom_app_identity() {
        let mut settings=crate::model::Settings::default();
        settings.avoid_apps=vec![(r"D:\Apps\Editor.exe".into(),"编辑器".into())];
        assert!(!avoidance_matches(r"D:\Apps\Editor.exe",&settings));
        settings.browser_minimal=true;
        assert!(avoidance_matches(r"D:\Quark\quark.exe",&settings));
        assert!(avoidance_matches(r"d:\apps\EDITOR.EXE",&settings));
        assert!(!avoidance_matches(r"C:\Other\Editor.exe",&settings));
        settings.avoid_browsers=false;
        assert!(!avoidance_matches(r"D:\Quark\quark.exe",&settings));
        assert!(avoidance_matches(r"D:\Apps\Editor.exe",&settings));
        settings.avoid_apps.clear();
        assert!(!avoidance_matches(r"D:\Apps\Editor.exe",&settings));
    }
    #[test]
    fn avoidance_window_list_excludes_this_process_and_deduplicates_apps() {
        let entries=avoidance_windows().unwrap();
        let current=std::env::current_exe().unwrap().to_string_lossy().into_owned();
        for (index,app) in entries.iter().enumerate() {
            assert!(!crate::avoidance::same_app(&app.path,&current));
            assert!(!app.name.is_empty() && !app.title.is_empty());
            assert!(!entries[..index].iter().any(|previous|crate::avoidance::same_app(&previous.path,&app.path)));
        }
        let notepad=std::path::Path::new(r"C:\Windows\System32\notepad.exe");
        if notepad.exists(){assert!(executable_description(notepad.to_str().unwrap()).is_some());}
    }
}
