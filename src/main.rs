#![cfg_attr(windows, windows_subsystem = "windows")]
//! Windows 外壳：无边框透明窗口 + 托盘 + 系统数据，界面全部交给 `lingqi::app::App`。

use eframe::egui::{self, Color32, Pos2};
use lingqi::app::{self, App, Cmd, Env, Options};
#[cfg(windows)]
use lingqi::poll_clock::PollClock;
use std::time::{Duration, Instant};

mod diagnostics;
mod graphics;

struct Shell {
    app: App,
    t0: Instant,
    #[cfg(windows)]
    frame_at: Instant,
    /// 下一帧之前大约需要等多久（由上一帧的重绘节奏决定）。
    repaint_hint: std::time::Duration,
    /// 兼容渲染上一次请求读回画面的时刻（无输入的持续动画按约 30 帧/秒读回）。
    #[cfg(windows)]
    readback_at: Option<Instant>,
    #[cfg(windows)]
    was_interactive_transition: bool,
    seek_rx: Option<(&'static str,std::sync::mpsc::Receiver<bool>)>,
    _resource_monitor:Option<lingqi::resources::Monitor>,
    resource_at:PollClock,
    guard_notified:bool,
    /// 刚点过播放控制或音量时，短时间内按最快频率采样，让按钮状态及时回显。
    placed: bool,
    placement_at: PollClock,
    placement_scale: f32,
    #[cfg(windows)]
    placed_bounds: Option<lingqi::display_layout::Bounds>,
    #[cfg(windows)]
    horizontal_width: f32,
    /// 上次定位时的目标屏幕、水平位置与 OLED 像素偏移；变化后立即重新定位，不等下一次轮询。
    placement_key: (String, i32, lingqi::model::Dock, bool, (i32, i32)),
    /// OLED 像素偏移当前实际使用的位移；展开、拖拽、隐藏时保持不变。
    #[cfg(windows)]
    oled_shift: (i32, i32),
    win_x: f32,
    sys: (Option<u8>, bool, PollClock),
    #[cfg(windows)]
    system_state: lingqi::system::Snapshot,
    #[cfg(windows)]
    system_ready: bool,
    #[cfg(windows)]
    system_rx: std::sync::mpsc::Receiver<lingqi::system::Observation>,
    fullscreen: Option<lingqi::display_layout::Bounds>,
    input_state:lingqi::input::Monitor,
    #[cfg(windows)]
    input_keys:Option<lingqi::input::Shortcuts>,
    #[cfg(all(windows, feature = "material"))]
    material: Option<lingqi::material::Runtime>,
    #[cfg(all(windows, feature = "material", feature = "material"))]
    material_debug: bool,
    #[cfg(all(windows, feature = "material", feature = "material"))]
    material_requested_id: Option<String>,
    #[cfg(all(windows, feature = "material", feature = "material"))]
    material_source: Option<String>,
    #[cfg(all(windows, feature = "material", feature = "material"))]
    material_failed: bool,
    #[cfg(windows)]
    tray: Option<lingqi::tray::Tray>,
    #[cfg(windows)]
    autostart: bool,
    #[cfg(windows)]
    hidden: bool,
    /// 毛玻璃皮肤的背景亮度采样（仅毛玻璃皮肤运行）。
    #[cfg(windows)]
    backdrop: Option<lingqi::backdrop::Sampler>,
    #[cfg(windows)]
    native: Option<lingqi::native::Window>,
    volume_quiet: PollClock,
    /// 置物架模块开启且记录剪贴板时才监听系统剪贴板。
    clipboard_watch: Option<lingqi::clipboard::Watcher>,
}

const MODULE_OFF:&str="该功能已在「页面管理」中关闭";

impl Shell {
    /// 按模块开关启停后台服务：关闭的模块不保留任何线程，重新开启时从头对齐配置。
    fn reconcile_modules(&mut self,ctx:&egui::Context){
        let clipboard_on=self.app.clipboard_active();
        if clipboard_on!=self.clipboard_watch.is_some() {
            self.clipboard_watch=if clipboard_on {let ctx=ctx.clone();lingqi::clipboard::watch(self.app.clipboard.clone(),move||ctx.request_repaint())}else{None};
        }
    }



    #[cfg(windows)]
    fn sync_capture_exclusion(app: &mut App, window: Option<&mut lingqi::native::Window>) -> bool {
        #[cfg(feature = "material")]
        if app.material_capture_forced {
            app.capture_exclusion_request = None;
            return false;
        }
        let was_enabled = app.settings.exclude_from_capture;
        let requested = app.capture_exclusion_request.take();
        let desired = requested.unwrap_or(was_enabled);
        let error = match window {
            Some(window) => window.take_capture_exclusion_error().or_else(|| {
                requested.and_then(|enabled| {
                    window.set_capture_exclusion(enabled);
                    window.take_capture_exclusion_error()
                })
            }),
            None => desired.then(|| "当前窗口无法应用截图和录屏隐藏。".into()),
        };
        let applied = desired && error.is_none();
        if was_enabled != applied {
            app.settings.exclude_from_capture = applied;
            app.settings.save();
        }
        let failed = error.is_some();
        if failed {
            if was_enabled && !app.settings_open {
                app.say("截图和录屏隐藏未生效");
            }
            app.capture_exclusion_error = error;
        }
        requested.is_some() || failed
    }

    #[cfg(all(windows, feature = "material"))]
    fn restore_capture_preference(app: &mut App, window: Option<&mut lingqi::native::Window>) {
        app.material_capture_forced = false;
        if let Some(window) = window {
            window.restore_capture_exclusion(app.settings.exclude_from_capture);
            app.capture_exclusion_error = window.take_capture_exclusion_error();
        }
    }

    #[cfg(all(windows, feature = "material", feature = "material"))]
    fn reconcile_material(&mut self, ctx: &egui::Context, state: Option<&eframe::egui_wgpu::RenderState>) {
        use lingqi::material::{Options, Runtime};

        if self.material_debug { return; }
        let spec = self.app.material_spec();
        let id = spec.as_ref().map(|spec| spec.id);
        let changed = id != self.material_requested_id.as_deref();
        if changed {
            self.material_requested_id = id.map(str::to_owned);
            self.material_failed = false;
        }
        // 通用设置的兼容渲染开关在重启时切换后端，期间沿用当前已加载的材质。
        let Some(spec) = spec.filter(|spec| self.app.settings.render_compat
            || self.app.material_active_id.as_deref() == Some(spec.id)) else {
            if self.material.take().is_some() {
                self.app.material_active_id = None;
                self.material_source = None;
                Self::restore_capture_preference(&mut self.app, self.native.as_mut());
                request_material_restart(&mut self.app, ctx, false);
            }
            return;
        };
        if self.material_failed { return; }
        let options = Options::from(&spec);
        if self.app.material_active_id.as_deref() == Some(spec.id)
            && self.material_source.as_deref() == Some(spec.source)
        {
            if let Some(material) = &mut self.material {
                material.set_options(options);
                return;
            }
        }
        let Some((_, state)) = self.native.as_ref().filter(|w| w.material_window().is_some()).zip(state) else {
            if changed {
                request_material_restart(&mut self.app, ctx, false);
            }
            return;
        };
        match Runtime::load(state, spec.source, options) {
            Ok(runtime) => {
                self.material_source = Some(spec.source.to_owned());
                self.app.material_active_id = Some(spec.id.to_owned());
                self.material = Some(runtime);
            }
            Err(error) => {
                self.material = None;
                self.app.material_active_id = None;
                self.material_source = None;
                self.material_failed = true;
                Self::restore_capture_preference(&mut self.app, self.native.as_mut());
                disable_material(&mut self.app, ctx, error, true);
            }
        }
    }

    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        diagnostics::graphics(cc);
        let running_compat=cc.wgpu_render_state.is_some();
        diagnostics::startup("加载本地配置");
        let mut app = App::new(&cc.egui_ctx, Options::default());
        #[cfg(all(windows, feature = "material"))]
        let material_debug = lingqi::material::debug_requested();
        app.render_compat_running=running_compat;
        diagnostics::startup("迁移配置");
        app.migrate_merged_switches();
        diagnostics::startup("启动剪贴板记录");
        if std::env::args().any(|arg|arg=="--settings"){app.open_settings();}
        diagnostics::startup("初始化媒体和托盘");
                    #[cfg(windows)]
        let autostart = lingqi::tray::autostart_enabled();
        #[cfg(windows)]
        let tray = if profiling_island(){None}else{app.start_tray(lingqi::tray::State {
            pet: app.settings.pet,
            minimal: app.settings.minimal,
            autostart,
        })};
        diagnostics::startup("初始化通知监听");
        #[cfg(windows)]
        #[cfg(windows)]
        // 音频、网络与输入法查询由后台完成，避免首帧等待系统服务。
        diagnostics::startup("创建后台采样与窗口");
        let initial_input=None;
        #[cfg(windows)] let system_state=lingqi::system::Snapshot::default();
        #[cfg(windows)] let system_rx=lingqi::system::observe(cc.egui_ctx.clone(),system_state.clone(),initial_input);
        // 岛体大小只作用于主窗口，在首帧之前生效，避免启动时先按 100% 闪一下。
        #[cfg(windows)] eframe::set_root_viewport_scale(app.settings.island_scale());
        #[cfg(windows)]
        let native = {
            let mut window = lingqi::native::Window::new(cc);
            #[cfg(feature = "material")]
            let material_window = window.as_ref().and_then(lingqi::native::Window::material_window).is_some();
            #[cfg(not(feature = "material"))]
            let material_window = false;
            if !material_window {
                if app.settings.exclude_from_capture {
                    app.capture_exclusion_request = Some(true);
                }
                Self::sync_capture_exclusion(&mut app, window.as_mut());
            }
            window
        };
        #[cfg(all(windows, feature = "material"))]
        if let Some(error) = lingqi::material::take_startup_failure() {
            #[cfg(feature = "material")]
            if !material_debug {
                persist_material_failure(&mut app);
            }
            app.alert("材质加载失败", error, 0, false);
        } else if lingqi::material::falling_back() {
            app.say("材质加载失败，已恢复普通绘制");
        } else {
            #[cfg(feature = "material")]
            if !material_debug
                && app.settings.render_compat
                && app.material_spec().is_some()
                && native.as_ref().and_then(lingqi::native::Window::material_window).is_none()
            {
                let error = "无法使用 Direct3D 12 透明交换链，已恢复普通绘制";
                lingqi::material::Runtime::report_failure(error);
                persist_material_failure(&mut app);
                app.alert("材质加载失败", error, 0, false);
            }
        }
        #[cfg(all(windows, feature = "material"))]
        let material = native.as_ref()
            .filter(|window| material_debug && window.material_window().is_some())
            .zip(cc.wgpu_render_state.as_ref())
            .and_then(|(_, state)| {
                match lingqi::material::Runtime::debug(state) {
                    Ok(runtime) => Some(runtime),
                    Err(error) => {
                        disable_material(&mut app, &cc.egui_ctx, error, false);
                        None
                    }
                }
            });
        #[cfg(all(windows, feature = "material", feature = "material"))]
        let material_requested_id = app.material_spec().map(|spec| spec.id.to_owned());
        Self {
            app,
            t0: Instant::now(),
            #[cfg(windows)]
            frame_at: Instant::now(),
            repaint_hint: std::time::Duration::from_secs_f64(1.0/60.0),
            #[cfg(windows)]
            readback_at: None,
            #[cfg(windows)]
            was_interactive_transition: false,
                seek_rx: None,
                    _resource_monitor:lingqi::resources::Monitor::start(lingqi::model::config_dir().join("diagnostics")),
            resource_at:PollClock::default(),
            guard_notified:false,
                    placed: false,
            placement_at: PollClock::default(),
            placement_scale: 0.0,
            #[cfg(windows)] placed_bounds:None,
            #[cfg(windows)] horizontal_width:app::WIN_W,
            placement_key: (String::new(), 0, lingqi::model::Dock::Top, true, (0, 0)),
            #[cfg(windows)]
            oled_shift: (0, 0),
            win_x: 0.0,
            sys: (None, false, PollClock::default()),
            #[cfg(windows)]
            system_state,
            #[cfg(windows)] system_ready:false,
            #[cfg(windows)] system_rx,
            fullscreen:None,
            input_state:lingqi::input::Monitor::new(initial_input),
            #[cfg(windows)] input_keys:lingqi::input::Shortcuts::start(),
            #[cfg(windows)] hidden:false,
            #[cfg(windows)] backdrop:None,
            #[cfg(windows)]
            native,
            #[cfg(all(windows, feature = "material"))]
            material,
            #[cfg(all(windows, feature = "material", feature = "material"))]
            material_debug,
            #[cfg(all(windows, feature = "material", feature = "material"))]
            material_requested_id,
            #[cfg(all(windows, feature = "material", feature = "material"))]
            material_source: None,
            #[cfg(all(windows, feature = "material", feature = "material"))]
            material_failed: false,
            #[cfg(windows)]
            tray,
            #[cfg(windows)]
            autostart,
            #[cfg(windows)]
            volume_quiet: PollClock::default(),
            clipboard_watch: None,
            }
    }

    #[cfg(windows)]
    fn poll_tray(&mut self) {
        use lingqi::tray::{self, Event};
        let events = self.tray.as_ref().map(|t| t.poll()).unwrap_or_default();
        for e in events {
            match e {
                Event::OpenSettings=>self.app.open_settings(),
                Event::OpenAi=>self.app.open_ai(),
                Event::TogglePet => {
                    self.app.set_pet(!self.app.settings.pet);
                    self.app.say(if self.app.settings.pet { "吉祥物已打开" } else { "吉祥物已关闭" });
                }
                Event::ToggleMinimal => {
                    self.app.set_minimal(!self.app.settings.minimal);
                    self.app.say(if self.app.settings.minimal { "极简刘海已打开" } else { "极简刘海已关闭" });
                }
                Event::ToggleAutostart => {
                    let want = !self.autostart;
                    match tray::set_autostart(want) {
                        Ok(()) => {
                            self.autostart = want;
                            self.app.say(if want { "已设为开机启动" } else { "已取消开机启动" });
                        }
                        Err(e) => self.app.say(e),
                    }
                }
            }
        }
        if let Some(t) = &self.tray {
            t.sync(tray::State { pet: self.app.settings.pet, minimal: self.app.settings.minimal, autostart: self.autostart });
        }
    }

    #[cfg(windows)]
    fn next_horizontal_width(&self, scale: f32) -> f32 {
        let available = self.placed_bounds.map_or(app::WIN_W, |bounds| {
            ((bounds.right - bounds.left) as f32 / scale).floor().max(app::WIN_W)
        });
        let wanted = self.app.requested_horizontal_width().max(self.horizontal_width);
        // 预留一小段余量，常驻数值更新时不必反复调整原生窗口。
        ((wanted / 64.0).ceil() * 64.0).min(available)
    }

    fn env(&mut self, ctx: &egui::Context) -> Env {
        let (clock, today, date) = local_now();
        // 电量变化很慢，每 5 秒读一次就够
        if self.sys.2.ready(Duration::from_secs(5)) {
            let (b, c) = read_power();
            if self.app.settings.show_battery && self.sys.0.is_some() && self.sys.1!=c && self.app.settings.system_hud {self.app.alert(if c {"电源已连接"}else{"已切换电池供电"},b.map(|v|format!("当前电量 {v}%")).unwrap_or_else(||"桌面电源状态已更新".into()),5,false);}
            if self.app.settings.show_battery && b.is_some_and(|v|v<=20) && !c && self.sys.0.is_some_and(|v|v>20) {self.app.alert("电量偏低",format!("剩余 {}% · 请连接电源",b.unwrap()),5,true);}
            self.sys = (b, c, PollClock::now());
        }
        let ppp = ctx.pixels_per_point();
        let origin = ctx.input(|i| i.viewport().outer_rect).map(|r| r.min).unwrap_or(Pos2::new(self.win_x, 0.0));
        #[cfg(windows)]
        let origin=self.native.as_ref().and_then(|w|w.logical_origin(ppp)).unwrap_or(origin);
        let desktop_cursor=desktop_cursor();
        let cursor=desktop_cursor.map(|(x,y)|Pos2::new(x as f32/ppp-origin.x,y as f32/ppp-origin.y));
        #[cfg(windows)]
        let cursor=cursor.filter(|_|!self.native.as_ref().is_some_and(|w|w.holding_through_strip()));
        // 所在屏幕换算到窗口逻辑坐标，供通知详情等浮层避开屏幕边缘。
        #[cfg(windows)]
        let screen=self.placed_bounds.map(|b|{
            egui::Rect::from_min_max(Pos2::new(b.left as f32/ppp-origin.x,b.top as f32/ppp-origin.y),Pos2::new(b.right as f32/ppp-origin.x,b.bottom as f32/ppp-origin.y))
        });
        #[cfg(not(windows))]
        let screen=None;
        #[cfg(windows)]
        let (backdrop,glass_blur)=(self.backdrop.as_ref().and_then(|b|b.color()),self.native.as_ref().is_some_and(|w|w.blur_active()));
        #[cfg(not(windows))]
        let (backdrop,glass_blur)=(None,false);
        #[cfg(windows)]
        let screen_locked=lingqi::system::screen_locked();
        #[cfg(not(windows))]
        let screen_locked=false;
        Env {
            t: self.t0.elapsed().as_secs_f64(),
            clock,
            today,
            date,
            battery: self.sys.0,
            charging: self.sys.1,
            network: Some(self.system_state.network),
            cursor,
            desktop_cursor,
            primary_down:{
                #[cfg(windows)] {self.app.dragging_position().then(||unsafe {
                    use windows::Win32::UI::{Input::KeyboardAndMouse::{GetAsyncKeyState,VK_LBUTTON,VK_RBUTTON},WindowsAndMessaging::{GetSystemMetrics,SM_SWAPBUTTON}};
                    let key=if GetSystemMetrics(SM_SWAPBUTTON)!=0 {VK_RBUTTON}else{VK_LBUTTON};
                    GetAsyncKeyState(key.0 as i32)<0
                })}
                #[cfg(not(windows))] {None}
            },
            cancel_position_drag:{
                #[cfg(windows)] {self.app.dragging_position() && unsafe {
                    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState,VK_ESCAPE};
                    GetAsyncKeyState(VK_ESCAPE.0 as i32)<0
                }}
                #[cfg(not(windows))] {false}
            },
            idle_seconds: {
                #[cfg(windows)] {Some(lingqi::system::idle_seconds())}
                #[cfg(not(windows))] {None}
            },
            screen_locked,
            recording: false,
            fullscreen_hidden: {
                #[cfg(windows)] {self.hidden}
                #[cfg(not(windows))] {false}
            },
            browser_at_top: {
                #[cfg(windows)] {if self.app.settings.browser_minimal {self.native.as_mut().and_then(|native|native.browser_at_top(&self.app.settings))}else{None}}
                #[cfg(not(windows))] {None}
            },
                volume: self.system_state.volume,
            screen,
            backdrop,
            glass_blur,
            opaque_surface: {
                #[cfg(windows)] {self.native.as_ref().is_some_and(|w|w.opaque_surface())}
                #[cfg(not(windows))] {false}
            },
            #[cfg(all(windows, feature = "material"))]
            material_active: self.material.is_some() && self.placed && !self.hidden && !screen_locked,
            #[cfg(all(windows, feature = "material"))]
            material_appearance: self.material.as_ref().map_or_else(Default::default, lingqi::material::Runtime::appearance),
        }
    }
}

impl eframe::App for Shell {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    #[cfg(windows)]
    fn raw_input_hook(&mut self, ctx:&egui::Context, input:&mut egui::RawInput) {
        if input.viewport_id != egui::ViewportId::ROOT || self.native.is_none() {return;}
        let size=if self.app.settings.dock.is_side() {
            egui::vec2(lingqi::side_dock::SIDE_W,lingqi::side_dock::SIDE_H)
        } else {
            let width = self.next_horizontal_width(ctx.pixels_per_point());
            if self.horizontal_width != width {
                self.horizontal_width = width;
                self.placement_at = PollClock::default();
            }
            egui::vec2(width + app::CANVAS_W - app::WIN_W, app::WIN_H)
        };
        input.screen_rect=Some(egui::Rect::from_min_size(Pos2::ZERO,size));
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let update_start=Instant::now();
        #[cfg(all(windows, feature = "material", feature = "material"))]
        self.reconcile_material(ctx, _frame.wgpu_render_state());
        #[cfg(windows)]
        lingqi::resources::FRAMES.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
        let frame_start=Instant::now();
        let mut marks:Vec<(&str,Duration)>=Vec::with_capacity(10);
        // 诊断用：这一帧里各处请求重绘的等待时长。
        macro_rules! mark {($name:expr)=>{marks.push(($name,frame_start.elapsed()));}}
        if self.resource_at.ready(Duration::from_secs(30)){
            let (count,bytes)={let textures=ctx.tex_manager();let textures=textures.read();(textures.num_allocated(),textures.allocated().map(|(_,info)|info.bytes_used()).sum())};
            lingqi::resources::TEXTURES.store(count,std::sync::atomic::Ordering::Relaxed);
            lingqi::resources::TEXTURE_BYTES.store(bytes,std::sync::atomic::Ordering::Relaxed);
            lingqi::resources::GALLEYS.store(ctx.fonts(|fonts|fonts.num_galleys_in_cache()),std::sync::atomic::Ordering::Relaxed);
            self.resource_at=PollClock::now();
        }
        // 设置里改了岛体大小：下一帧起主窗口按新倍率栅格化，定位逻辑随像素比变化同步调整画布。
        #[cfg(windows)] eframe::set_root_viewport_scale(self.app.settings.island_scale());
        #[cfg(windows)]
        thread_local!{static AFTER_INTERACTIVE_CAPTURE:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
        #[cfg(windows)]
        if let Some(start)=_frame.interactive_frame_start {
            self.frame_at=start;
            AFTER_INTERACTIVE_CAPTURE.set(true);
        } else if AFTER_INTERACTIVE_CAPTURE.replace(false) {
            // 交互截图帧之后紧跟的这一帧负责把读回画面交给分层窗口，只隔几毫秒就会到来。
            // 再按帧间隔睡一次会错过下一次屏幕刷新，切页等动画掉到 30 帧；节奏已由交互截图的截止时间控制。
            self.frame_at=Instant::now();
        } else {
            // 节奏交给 eframe：它按应用请求的重绘时间点等待，并在有输入时立刻唤醒。
            // 这里再睡一次只会拖慢每一帧（动画帧率被腰斩），而且挡不住指针响应。
            self.frame_at=Instant::now();
        }
        mark!("帧间隔等待");
        #[cfg(windows)]
        self.poll_tray();
        #[cfg(windows)]
        let foreground_changed=self.native.as_mut().is_some_and(|window|window.take_foreground_change());
        #[cfg(windows)]
        if foreground_changed {
            // 切到别的程序：收起，并在光标离开前不再自动展开
            self.app.foreground_changed(ctx);
        }
        #[cfg(windows)]
        let offset=self.app.placement_offset();
        #[cfg(windows)]
        let shift=self.oled_shift;
        // 拖动时位置不取整，按万分之一比例比较，窗口逐帧平滑跟随。
        #[cfg(windows)]
        if self.native.as_ref().is_some_and(|w|w.take_display_change()) {self.placement_at=PollClock::default();}
        #[cfg(windows)]
        let placement_key=(self.app.settings.display_id.clone(),(offset*10000.0).round() as i32,self.app.settings.dock,self.app.settings.avoids_top_bar(),shift);
        #[cfg(windows)]
        let refresh=self.placement_at.ready(Duration::from_secs(1));
        #[cfg(windows)]
        if refresh || self.placement_key.0!=placement_key.0 {
            self.placement_at=PollClock::now();
            self.app.displays=lingqi::display_layout::connected();
        }
        #[cfg(windows)]
        let display=lingqi::display_layout::selected(&self.app.displays,&self.app.settings.display_id);
        #[cfg(windows)]
        let bounds=display.map(|d|d.dock_bounds(self.app.settings.dock,self.app.settings.avoids_top_bar()));
        #[cfg(windows)]
        if self.app.settings.dock==lingqi::model::Dock::Bottom && display.is_some_and(|d|d.auto_bottom!=0) && !self.hidden {
            ctx.request_repaint_after(Duration::from_millis(32));
        }
        #[cfg(windows)]
        if !self.placed || refresh || self.placed_bounds!=bounds || self.placement_scale!=ctx.pixels_per_point() || self.placement_key!=placement_key {
            self.placement_scale=ctx.pixels_per_point();
            let moving_display=self.placed && (self.app.dragging_position() || self.placement_key.0!=placement_key.0);
            self.placement_key=placement_key;
            let scale=display.filter(|_|moving_display).map_or(self.placement_scale,|d|d.scale(self.app.settings.island_scale()*ctx.zoom_factor()));
            if bounds.is_some_and(|b|self.native.as_mut().is_some_and(|w|w.place_on_display(scale,b,offset,self.app.settings.dock,shift,self.horizontal_width))) {
                self.placed_bounds=bounds;
                if !self.placed {
                    diagnostics::startup("目标屏幕物理坐标定位完成，已请求显示");
                    self.placed=true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                }
            } else if self.t0.elapsed()>=Duration::from_secs(2) {
                // Do not leave a hidden startup window forever when monitor discovery fails.
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            }
        }
        mark!("定位");
        #[cfg(not(windows))]
        if !self.placed {
            if let Some(mon)=ctx.input(|i|i.viewport().monitor_size) {
                self.placed=true;self.win_x=((mon.x-app::WIN_W)/2.0).round();
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(self.win_x,0.0)));
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            }
        }
        #[cfg(windows)]
        {
            while let Ok(observation)=self.system_rx.try_recv(){
                let next=observation.input;
                let manual=next.is_some_and(|state|self.input_keys.as_ref().is_some_and(|keys|keys.allows(state.context)));
                if let Some(label)=self.input_state.observe(next,manual){
                    if let Some(keys)=&self.input_keys{keys.clear();}
                    if self.app.settings.system_hud{self.app.alert("键盘输入语言",label,5,false);}
                }
                let new=observation.system;
                // 切换输出设备时，系统会恢复该设备的音量；等待它稳定后再提示调节。
                if new.audio_endpoint!=self.system_state.audio_endpoint {self.volume_quiet=PollClock::now();}
                if self.system_ready && self.app.settings.system_hud {
                    for (title,body) in new.changes(&self.system_state) {
                        // 拖动音量条刚改过音量时，系统回声不再单独弹一条。
                        if title=="系统音量" && !self.volume_quiet.ready(Duration::from_millis(900)) { continue; }
                        self.app.alert(title,body,5,false);
                    }
                }
                self.system_state=new;
                self.system_ready=true;
                self.fullscreen=observation.fullscreen;
            }
            self.hidden=!self.app.dragging_position() && lingqi::display_layout::hide_for_fullscreen(self.app.settings.hide_fullscreen,self.fullscreen,lingqi::display_layout::selected(&self.app.displays,&self.app.settings.display_id).map(|d|d.bounds));
            // 毛玻璃：在岛体左右两侧和画布底边（都在窗口区域之外）取样，估计岛背后的明暗。
            if self.app.effective_skin().is_glass() && !self.hidden {
                let sampler=self.backdrop.get_or_insert_with(lingqi::backdrop::Sampler::start);
                let ppp=ctx.pixels_per_point();
                if let Some(origin)=self.native.as_ref().and_then(|w|w.logical_origin(ppp)) {
                    let points=self.app.backdrop_points(self.horizontal_width);
                    sampler.set_points(points.iter().map(|&(x,y)|(((origin.x+x)*ppp).round() as i32,((origin.y+y)*ppp).round() as i32)).collect());
                }
            } else {
                self.backdrop=None;
            }
            mark!("系统状态与取样");
    }
        if let Some((message,rx))=&self.seek_rx {
            if let Some(success)=match rx.try_recv(){Ok(success)=>Some(success),Err(std::sync::mpsc::TryRecvError::Disconnected)=>Some(false),Err(_)=>None}{
                let message=*message;
                self.seek_rx=None;
                if !success{self.app.say(message);}
            }
        }
        mark!("消息客户端");
        let env = self.env(ctx);
        mark!("环境");
        #[cfg(windows)]
        if self.native.as_ref().is_some_and(|w|w.page_scroll_cancelled()) {self.app.cancel_page_scroll();}
        self.reconcile_modules(ctx);
        mark!("模块对齐");
        mark!("模块状态");
        let mut out = None;
        let pre_cost=frame_start.elapsed();
        let pre_parts={let mut last=Duration::ZERO;marks.iter().filter_map(|(name,at)|{let part=at.saturating_sub(last);last=*at;(part>=Duration::from_micros(500)).then(||format!("{name} {:.1}",part.as_secs_f64()*1000.0))}).collect::<Vec<_>>().join("、")};
        #[cfg(all(windows, feature = "material"))]
        if !env.material_active {
            if let Some(material) = &mut self.material { material.suspend(); }
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::TRANSPARENT))
            .show(ctx, |ui| {
                // 左右停靠时画布是竖向的 SIDE_W × SIDE_H。
                #[cfg(windows)]
                let width = self.horizontal_width;
                #[cfg(not(windows))]
                let width = app::WIN_W;
                let size=if self.app.settings.dock.is_side() {egui::vec2(lingqi::side_dock::SIDE_W,lingqi::side_dock::SIDE_H)} else {egui::vec2(width,app::WIN_H)};
                let win = egui::Rect::from_min_size(ui.max_rect().min,size);
                let mut draw = || self.app.frame(ui, win, &env);
                #[cfg(all(windows, feature = "material"))]
                if let Some(material) = self.material.as_mut().filter(|_| env.material_active) {
                    out = Some(material.draw(draw));
                    return;
                }
                out = Some(draw());
            });
        let Some(out) = out else { return };
        #[cfg(windows)]
        if !self.app.settings.dock.is_side() && self.next_horizontal_width(ctx.pixels_per_point()) != self.horizontal_width {
            ctx.request_repaint();
        }
        let draw_cost=frame_start.elapsed()-pre_cost;
        #[allow(unused_mut)]
        let mut native_cost=Duration::ZERO;
        #[cfg(all(windows, feature = "material"))]
        let mut out = out;
        #[cfg(windows)]
        // 页面自己驱动的动画（天气日出日落、性能详情、翻页后的页面缓动）靠 egui 立即重绘推进，
        // 兼容渲染下也按交互帧交付。插件状态页已经在 FrameOut 标记画布动画，截图回调的重绘只负责交付。
        let page_animating = self.app.render_compat_running && out.expanded
            && !self.app.is_plugin_status_page() && ctx.requested_repaint_last_pass();
        #[cfg(windows)]
        let interactive_transition = !self.hidden && (out.interactive_transition || page_animating);
        #[cfg(windows)]
        let last_transition_frame = std::mem::replace(&mut self.was_interactive_transition,interactive_transition);

        #[cfg(windows)]
        {
            // 先处理本帧输入和动画，再决定是否移动，避免悬停或按下的首帧仍使用旧状态。
            let hovering=env.cursor.is_some_and(|p|out.hit.contains(p)||out.notice_hit.is_some_and(|r|r.contains(p)));
            let shift=if !self.app.settings.oled_shift {(0,0)}
                else if !self.hidden && !hovering && self.app.oled_shift_idle(ctx) {
                    ctx.request_repaint_after(lingqi::oled_shift::until_next_step(self.t0.elapsed()));
                    let target=lingqi::oled_shift::shift(true,self.app.settings.dock,self.t0.elapsed());
                    lingqi::oled_shift::toward(self.oled_shift,target)
                } else {self.oled_shift};
            if shift!=self.oled_shift {
                self.oled_shift=shift;
                let display=lingqi::display_layout::selected(&self.app.displays,&self.app.settings.display_id);
                if display.is_some_and(|d|self.native.as_mut().is_some_and(|w|w.place_on_display(ctx.pixels_per_point(),d.dock_bounds(self.app.settings.dock,self.app.settings.avoids_top_bar()),self.app.placement_offset(),self.app.settings.dock,shift,self.horizontal_width))) {
                    self.placement_key.4=shift;
                }
                ctx.request_repaint();
            }
        }

        #[cfg(windows)]
        if let Some(native) = &mut self.native {
            native.begin_crop_frame(_frame.root_gl_front_ready);
            // 兼容渲染：读回的画面交给分层窗口，边缘保留抗锯齿。画面变化（或有其他输入）时才请求下一次读回，
            // 读回完成本身触发的那一帧若画面没变就停下，静止时不会一直空转。
            if native.wants_frames() {
                let (shot,other_input)=ctx.input(|i|(
                    i.events.iter().rev().find_map(|e|match e{egui::Event::Screenshot{image,..}=>Some(image.clone()),_=>None}),
                    i.events.iter().any(|e|!matches!(e,egui::Event::Screenshot{..})),
                ));
                let changed=shot.as_ref().is_some_and(|image|native.present_frame(image.size,image.as_raw()));
                let startup_capture=(!self.hidden && native.waiting_for_first_layered_frame())
                    .then(||self.frame_at+Duration::from_secs_f64(1.0/60.0));
                // 若首张截图没产生回调，也继续尝试，直到正常交接或走轮廓回退。
                if let Some(deadline)=startup_capture {schedule_repaint(ctx,deadline.saturating_duration_since(Instant::now()));}
                // 动画刚停稳时也交付最后一帧，避免最终位置延迟到下一次闲置刷新。
                let capture_transition=interactive_transition || (last_transition_frame && !self.hidden);
                let interactive=other_input || capture_transition || startup_capture.is_some();
                if shot.is_none() || changed || interactive {
                    // 输入结束后，短暂的切换过渡仍需逐帧交付。
                    let now=Instant::now();
                    match lingqi::layered::readback_delay(self.readback_at,now,interactive,out.repaint_after) {
                        None=>{
                            self.readback_at=Some(now);
                            let data=if let Some(deadline)=startup_capture {
                                egui::UserData::new(eframe::InteractiveCapture(deadline))
                            } else if capture_transition && self.app.render_compat_running {
                                egui::UserData::new(eframe::InteractiveCapture(self.frame_at+Duration::from_secs_f64(1.0/60.0)))
                            } else {Default::default()};
                            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(data));
                        }
Some(wait)=>ctx.request_repaint_after(wait),
                    }
                }
            }
            let r = |r: egui::Rect| (r.left(), r.top(), r.width(), r.height());
            // 保留主窗口的事件循环，避免全屏隐藏后设置窗口无法关闭或重新打开。
            let region=if self.hidden {(0.0,0.0,0.0,0.0)} else {r(out.region)};
            let hit=if self.hidden || out.click_through && out.notice_hit.is_none() {(0.0,0.0,0.0,0.0)} else {r(out.hit)};
            let ppp=ctx.pixels_per_point();
            let glass=if self.hidden {None}else{out.glass.as_deref()};
            #[cfg(all(windows, feature = "material"))]
            let glass=glass.filter(|_| self.material.is_none());
            // 系统模糊要铺满整块轮廓，开着时不收紧窗口外框。
            let tight=(!self.hidden && native.supports_compact_bounds() && !out.blur
                && !self.app.settings.minimal && !out.click_through).then(||{
                let painted=ctx.paint_bounds();
                if painted.is_positive() {
                    painted.expand(2.0/ppp).union(out.hit).union(out.notice_hit.unwrap_or(egui::Rect::NOTHING)).intersect(out.region)
                } else {out.region}
            });
            native.request_crop(tight,out.geometry_transitioning);
            let native_start=Instant::now();
            if let Some(wait)=native.update(region, hit, if self.hidden{None}else{out.notice_hit.map(r)},if self.hidden{None}else{out.file_drop_zone.map(r)},glass,out.blur,ppp, out.editing && !self.hidden, !self.hidden && out.expanded,if self.hidden {None}else{out.resident_clip.as_deref()}) {

                ctx.request_repaint_after(wait);
            }
            let pill_hover=(!self.hidden && out.resident_clip.is_some()
                && self.app.settings.island_shape==lingqi::model::IslandShape::Pill).then_some(out.hit);
            native.watch_collapsed_pill_edge(pill_hover,ppp,self.app.settings.dock);
            native_cost=native_start.elapsed();
            _frame.root_window_offset=native.paint_offset();
            native.set_page_scroll_area(out.page_scroll_area.filter(|_|!self.hidden && out.expanded),ppp);
            if !self.hidden && self.app.settings.dock==lingqi::model::Dock::Top {
                let on_top=self.app.settings.avoid_top_bar && self.app.settings.top_bar_on_top;
                if let Some(display)=lingqi::display_layout::selected(&self.app.displays,&self.app.settings.display_id)
                    .filter(|_|on_top || out.top_bar_notice.is_some()) {
                    let notice=out.top_bar_notice.filter(|_|!on_top).map(|rect|(rect,ppp));
                    native.keep_above_top_bar(display.bounds,notice);
                }
            }
        }
        #[cfg(all(windows, feature = "material"))]
        if let Some((material, window)) = self.material.as_mut().zip(self.native.as_mut()) {
            // 原生定位、裁剪及样式更新完毕后，再确认截图排除和采样位置。
            match material.finish(ctx, window, &mut out, env.t, env.desktop_cursor) {
                Ok(delay) => {
                    self.app.material_capture_forced |= material.capture_ready();
                    if let Some(delay) = delay { schedule_repaint(ctx, delay); }
                }
                Err(error) => {
                    self.material = None;
                    let installed = self.app.material_active_id.take().is_some();
                    #[cfg(feature = "material")]
                    if installed {
                        self.material_source = None;
                        self.material_failed = true;
                    }
                    Self::restore_capture_preference(&mut self.app, self.native.as_mut());
                    disable_material(&mut self.app, ctx, error, installed);
                }
            }
        }
        for cmd in out.cmds {
            match cmd {
                #[cfg(windows)]
                Cmd::OpenPluginNotice{source,id,session}=>{
                }
                Cmd::RestartApp=>{
                    // 先拉起新进程（它会等本进程退出），再像托盘「退出」一样摘图标后结束。
                    if diagnostics::spawn_replacement(){
                        self.app.save_window_before_exit();
                        #[cfg(windows)] if let Some(tray)=&self.tray {tray.exit_process();}
                        std::process::exit(0);
                    }else{self.app.say("无法重新启动灵栖，请从托盘退出后手动打开");}
                }
                Cmd::Open(p) => {if let Err(e)=open_path(&p){self.app.say(e);}},
                Cmd::OpenUrl(url)=>{
                    if open_path(std::path::Path::new(&url)).is_err(){self.app.say("无法打开默认浏览器，请检查 Windows 默认应用设置");}
                },
                Cmd::AddFiles=>{
                    #[cfg(windows)] match lingqi::system::pick_files(){Ok(files)=>if !files.is_empty(){self.app.on_drop(files);self.app.show_page(3);},Err(e)=>self.app.say(format!("无法选择文件：{}",e.code()))}
                },
                Cmd::DragFile(p)=>{
                    #[cfg(windows)] if let Err(e)=lingqi::system::drag_file(&p){self.app.say(format!("无法拖出文件：{}",e.code()));}
                    ctx.input_mut(|i| i.pointer=Default::default());
                },
                Cmd::Reveal(p)=>{let _=lingqi::child_process::spawn(std::process::Command::new("explorer.exe").arg(format!("/select,{}",p.display())));},
                Cmd::ToggleAutostart=>{
                    #[cfg(windows)] {let want=!self.autostart;match lingqi::tray::set_autostart(want){Ok(())=>self.autostart=want,Err(e)=>self.app.say(e)}}
                },
                Cmd::SetVolume(v)=>{
                    #[cfg(windows)] lingqi::system::set_volume(v);
                    self.volume_quiet=PollClock::now();
                },
                Cmd::ToggleMute=>{
                    #[cfg(windows)] lingqi::system::toggle_mute();
                    self.volume_quiet=PollClock::now();
                },
                Cmd::Copy(text)=>{ctx.copy_text(text);self.app.say("已复制");},
                Cmd::Notify(title,body,sound)=>{
                    #[cfg(windows)] {
                        if sound==app::NotificationSound::Health {lingqi::tray::play_health_sound();}
                        if let Some(tray)=&self.tray {tray.notify(&title,&body,sound==app::NotificationSound::System);}
                    }
                },
                // 模块关闭时设置项已置灰；万一仍有命令到达，只提示，不为它临时启动服务。
                Cmd::Focus => ctx.send_viewport_cmd(egui::ViewportCommand::Focus),
            }
        }


        #[cfg(windows)] let autostart = self.autostart;
        #[cfg(not(windows))] let autostart = false;
        let settings_start=Instant::now();
        self.app.settings_ui(ctx, autostart);
        let settings_cost=settings_start.elapsed();
        #[cfg(windows)]
        if Self::sync_capture_exclusion(&mut self.app, self.native.as_mut()) {
            ctx.request_repaint();
        }
        // 交互切换以帧起点为准，扣掉本帧已经花费的时间。
        #[cfg(windows)] let repaint_after=if self.hidden {Duration::from_secs(1)}
            else if interactive_transition {out.repaint_after.saturating_sub(self.frame_at.elapsed())}
            else {out.repaint_after};
        #[cfg(not(windows))] let repaint_after=out.repaint_after;
        self.repaint_hint=repaint_after;
        schedule_repaint(ctx,repaint_after);
        // 只记慢帧：卡顿反馈时能直接看出是渲染慢还是输入被吞（页面/展开状态一并记下）。
        let elapsed=frame_start.elapsed();
        if elapsed>=Duration::from_millis(60) {
            let (page,open)=self.app.debug_state();
            lingqi::resources::append_log(&lingqi::model::config_dir().join("diagnostics"),"frames",
                &format!("帧 {:.0} ms｜页面 {}｜展开 {}｜前置 {:.0}｜岛体 {:.0}（插件 {:.1}）｜窗口 {:.0}｜设置 {:.0}",
                    elapsed.as_secs_f64()*1000.0,page,open,pre_cost.as_secs_f64()*1000.0,draw_cost.as_secs_f64()*1000.0,
                    lingqi::resources::PLUGIN_FRAME_US.load(std::sync::atomic::Ordering::Relaxed) as f64/1000.0,
                    native_cost.as_secs_f64()*1000.0,settings_cost.as_secs_f64()*1000.0));
        }
        // 设了 LINGQI_FRAME_TRACE 时逐帧记下动画帧：上一帧逻辑和逻辑之外（绘制、读回、提交、等待）各占多少。
        thread_local!{
            static PREVIOUS_FRAME:std::cell::Cell<Option<(Instant,Duration)>>=const{std::cell::Cell::new(None)};
            static PREVIOUS_PARTS:std::cell::Cell<[Duration;4]>=const{std::cell::Cell::new([Duration::ZERO;4])};
        }
        let parts=PREVIOUS_PARTS.replace([pre_cost,settings_cost,elapsed.saturating_sub(pre_cost+draw_cost+native_cost+settings_cost),update_start.elapsed().saturating_sub(elapsed)]);
        // 动画帧会请求很快的下一帧；闲置和定时刷新不计。
        let animating=out.repaint_after<=Duration::from_millis(34)
            || ctx.requested_repaint_last_pass() && std::env::var_os("LINGQI_FRAME_TRACE").is_some();
        if let Some((start,logic))=PREVIOUS_FRAME.get().filter(|_|animating) {
            let interval=frame_start.duration_since(start);
            if std::env::var_os("LINGQI_FRAME_TRACE").is_some() {
                let (page,open)=self.app.debug_state();
                lingqi::resources::append_log(&lingqi::model::config_dir().join("diagnostics"),"frames",
&format!("间隔 {:.0} ms｜页面 {}｜展开 {}｜上帧逻辑 {:.1}（前置 {:.1}、设置窗口 {:.1}、其余 {:.1}、材质与提醒 {:.1}）｜逻辑之外 {:.1}｜本帧岛体 {:.1}（插件 {:.1}）｜窗口 {:.1}｜本帧前置：{}｜过渡 {} 交互起点 {} 兼容 {}",
                        interval.as_secs_f64()*1000.0,page,open,logic.as_secs_f64()*1000.0,
                        parts[0].as_secs_f64()*1000.0,parts[1].as_secs_f64()*1000.0,parts[2].as_secs_f64()*1000.0,parts[3].as_secs_f64()*1000.0,
                        interval.saturating_sub(logic).as_secs_f64()*1000.0,draw_cost.as_secs_f64()*1000.0,
                        lingqi::resources::PLUGIN_FRAME_US.load(std::sync::atomic::Ordering::Relaxed) as f64/1000.0,
                        native_cost.as_secs_f64()*1000.0,pre_parts,out.interactive_transition,
                        _frame.interactive_frame_start.is_some(),self.app.render_compat_running));
            }
        }
        PREVIOUS_FRAME.set(Some((frame_start,elapsed)));
    }
}




// ---------------- 系统信息 ----------------

fn schedule_repaint(ctx:&egui::Context,interval:Duration){
    // egui 会预减一帧；本程序自行调度刷新，补回后才能保持实际的等待间隔。
    let predicted=ctx.input(|i|Duration::from_secs_f32(i.predicted_dt));
    ctx.request_repaint_after(interval.saturating_add(predicted));
}

#[cfg(test)]
mod repaint_tests {
    use super::*;

    fn scheduled_delay(interval:Duration,immediate:bool)->Duration{
        let ctx=egui::Context::default();
        let mut delay=Duration::ZERO;
        for frame in 0..4 {
            let output=ctx.run(egui::RawInput{time:Some(frame as f64),predicted_dt:1.0/60.0,..Default::default()},|ctx|{
                if immediate{ctx.request_repaint();}
                schedule_repaint(ctx,interval);
            });
            delay=output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        }
        delay
    }

    #[test]
    fn background_repaint_keeps_the_requested_frame_interval(){
        for fps in [60.0,30.0,20.0,10.0,1.0] {
            let interval=Duration::from_secs_f64(1.0/fps);
            let actual=scheduled_delay(interval,false);
            assert!(actual.abs_diff(interval)<Duration::from_micros(1),"目标 {fps} 帧不应被提前唤醒：{actual:?}");
        }
    }

    #[test]
    fn interaction_and_service_events_still_repaint_immediately(){
        assert_eq!(scheduled_delay(Duration::from_secs(1),true),Duration::ZERO);
    }
}

#[cfg(windows)]
fn local_now() -> (String, String, String) {
    use windows::Win32::System::SystemInformation::GetLocalTime;
    let st = unsafe { GetLocalTime() };
    let wd = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
    (
        format!("{:02}:{:02}:{:02}", st.wHour, st.wMinute, st.wSecond),
        format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay),
        format!("{}月{}日 {}", st.wMonth, st.wDay, wd[(st.wDayOfWeek as usize).min(6)]),
    )
}

#[cfg(not(windows))]
fn local_now() -> (String, String, String) {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (h, m, s) = ((secs / 3600) % 24, (secs / 60) % 60, secs % 60);
    (format!("{h:02}:{m:02}:{s:02}"), "1970-01-01".into(), "1月1日 周四".into())
}

#[cfg(windows)]
fn read_power() -> (Option<u8>, bool) {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut st = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut st) }.is_ok() {
        let pct = if st.BatteryLifePercent == 255 { None } else { Some(st.BatteryLifePercent) };
        (pct, st.ACLineStatus == 1)
    } else {
        (None, false)
    }
}

#[cfg(not(windows))]
fn read_power() -> (Option<u8>, bool) {
    (None, true)
}

#[cfg(windows)]
fn desktop_cursor() -> Option<(i32,i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) }.ok()?;
    Some((p.x,p.y))
}

#[cfg(not(windows))]
fn desktop_cursor() -> Option<(i32,i32)> {
    None
}

#[cfg(windows)]
fn open_path(p: &std::path::Path)->Result<(),String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{ShellExecuteExW,SHOpenWithDialog,SHELLEXECUTEINFOW,OPENASINFO,SEE_MASK_FLAG_NO_UI,OAIF_EXEC};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let mut wide: Vec<u16> = p.as_os_str().encode_wide().collect();
    wide.push(0);
    let mut info=SHELLEXECUTEINFOW{cbSize:std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,fMask:SEE_MASK_FLAG_NO_UI,lpVerb:windows::core::w!("open"),lpFile:PCWSTR(wide.as_ptr()),nShow:SW_SHOWNORMAL.0,..Default::default()};
    open_path_result(p,unsafe{ShellExecuteExW(&mut info)},||unsafe{
        SHOpenWithDialog(None,&OPENASINFO{pcszFile:PCWSTR(wide.as_ptr()),oaifInFlags:OAIF_EXEC,..Default::default()})
    })
}

#[cfg(windows)]
fn open_path_result(p:&std::path::Path,result:windows::core::Result<()>,choose:impl FnOnce()->windows::core::Result<()>)->Result<(),String>{
    use windows::Win32::Foundation::{ERROR_CANCELLED,ERROR_NO_ASSOCIATION};
    let result=match result{
        Err(error) if error.code()==windows::core::HRESULT::from_win32(ERROR_NO_ASSOCIATION.0)&&p.is_file()=>choose(),
        other=>other,
    };
    match result{
        Err(error) if error.code()==windows::core::HRESULT::from_win32(ERROR_CANCELLED.0)=>Ok(()),
        other=>other.map_err(|_|"无法打开，请检查文件位置和默认应用".into()),
    }
}

#[cfg(not(windows))]
fn open_path(_p: &std::path::Path)->Result<(),String> {Ok(())}

#[cfg(all(test,windows))]
mod open_path_tests {
    use super::*;
    use windows::Win32::Foundation::{ERROR_ACCESS_DENIED,ERROR_CANCELLED,ERROR_FILE_NOT_FOUND,ERROR_NO_ASSOCIATION};
    fn error(code:windows::Win32::Foundation::WIN32_ERROR)->windows::core::Error{windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(code.0))}

    #[test]
    fn unassociated_files_offer_open_with_and_cancellation_is_normal(){
        let path=std::env::temp_dir().join(format!("灵栖 无关联-{}.lingqi-open-test",std::process::id()));
        std::fs::write(&path,b"fixture").unwrap();
        for cancel in [false,true]{
            let mut chosen=false;
            let result=open_path_result(&path,Err(error(ERROR_NO_ASSOCIATION)),||{chosen=true;if cancel{Err(error(ERROR_CANCELLED))}else{Ok(())}});
            std::fs::remove_file(&path).unwrap();
            assert!(chosen,"文件没有默认关联时应提供系统打开方式");
            assert_eq!(result,Ok(()),"用户取消选择不应显示打开失败");
            if !cancel{std::fs::write(&path,b"fixture").unwrap();}
        }
    }

    #[test]
    fn default_apps_urls_and_other_errors_never_open_the_file_picker(){
        let file=std::env::temp_dir().join(format!("lingqi-open-errors-{}",std::process::id()));
        std::fs::write(&file,b"fixture").unwrap();
        for result in [Ok(()),Err(error(ERROR_ACCESS_DENIED)),Err(error(ERROR_FILE_NOT_FOUND))]{
            let success=result.is_ok();
            let actual=open_path_result(&file,result,||panic!("已有默认程序或其他打开错误不应弹出选择器"));
            assert_eq!(actual.is_ok(),success);
        }
        assert!(open_path_result(&file,Err(error(ERROR_NO_ASSOCIATION)),||Err(error(ERROR_ACCESS_DENIED))).is_err(),"选择器失败应保留错误提示");
        std::fs::remove_file(file).unwrap();
        for (path,result,success) in [
            ("https://example.com",Ok(()),true),
            ("https://example.com",Err(error(ERROR_NO_ASSOCIATION)),false),
            ("missing.lingqi-open-test",Err(error(ERROR_NO_ASSOCIATION)),false),
            ("missing.lingqi-open-test",Err(error(ERROR_FILE_NOT_FOUND)),false),
            ("blocked.lingqi-open-test",Err(error(ERROR_ACCESS_DENIED)),false),
        ]{
            let actual=open_path_result(std::path::Path::new(path),result,||panic!("非文件关联问题不能弹出打开方式"));
            assert_eq!(actual.is_ok(),success);
        }
    }
}

fn main() -> std::process::ExitCode {
    #[cfg(any(feature="sdk-js",feature="sdk-wasm"))]
    if let Some(code)=lingqi::sdk_spike::command(){return code;}
    diagnostics::install();
    // 收集诊断：在单实例检查、创建窗口和初始化显卡之前处理，灵栖启动即崩溃、卡死时也能用。
    #[cfg(windows)]
    if lingqi::diagnostics_tool::requested(std::env::args()) {
        return match lingqi::diagnostics_tool::run(&lingqi::model::config_dir()) {
            Ok(_) => std::process::ExitCode::SUCCESS,
            Err(error) => {diagnostics::report("诊断收集未完成。", &error); std::process::ExitCode::FAILURE}
        };
    }
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            diagnostics::report("灵栖启动或运行失败。", &error.to_string());
            std::process::ExitCode::FAILURE
        }
    }
}

// 性能复测使用独立数据目录，避开日常灵栖的托盘和安装信息。
fn profiling_island()->bool {
    cfg!(feature="sdk-js") && std::env::var_os("LINGQI_DATA_DIR").is_some() && std::env::args().any(|a|a=="--sdk-profile-island")
}

#[cfg(all(windows, feature = "material"))]
fn replace_material_window(fallback: bool) -> bool {
    let args: &[&str] = match (fallback, profiling_island()) {
        (true, true) => &[lingqi::material::FALLBACK_ARG, "--sdk-profile-island"],
        (true, false) => &[lingqi::material::FALLBACK_ARG],
        (false, true) => &["--sdk-profile-island"],
        (false, false) => &[],
    };
    diagnostics::spawn_replacement_with_args(args)
}

#[cfg(all(windows, feature = "material"))]
fn request_material_restart(app: &mut App, ctx: &egui::Context, fallback: bool) -> bool {
    if replace_material_window(fallback) {
        app.save_window_before_exit();
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        true
    } else {
        app.say("无法重新启动灵栖，请从托盘退出后手动打开");
        ctx.request_repaint();
        false
    }
}

#[cfg(all(windows, feature = "material", feature = "material"))]
fn persist_material_failure(app: &mut App) {
    if let Err(error) = app.deactivate_material() {
        lingqi::material::Runtime::report_failure(&format!("无法保存材质停用状态：{error}"));
    }
}

#[cfg(all(windows, feature = "material"))]
fn disable_material(app: &mut App, ctx: &egui::Context, error: String, _installed: bool) {
    lingqi::material::Runtime::report_failure(&error);
    #[cfg(feature = "material")]
    if _installed {
        persist_material_failure(app);
    }
    if !request_material_restart(app, ctx, true) {
        app.alert("材质加载失败", error, 0, false);
    }
}

/// 自研渲染后端：不用 eframe，自己建窗口与 DirectComposition 交换链。
///
/// 失败时由调用方回退到 egui，并把原因写进诊断——不能让用户对着一个空窗口发呆。
#[cfg(windows)]
fn run_self_backend() -> Result<(), String> {
    use lingqi::render::{render_loop, Host, IslandWindow};
    let settings = lingqi::model::Settings::load();
    let scale = settings.island_scale();
    let width = (app::CANVAS_W * scale).round() as i32;
    let height = (app::WIN_H * scale).round() as i32;
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN};
    let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let x = ((screen_width - width) / 2).max(0);
    let origin_x = x as f32 / scale;
    let window = IslandWindow::new(width, height, x, 0)?;
    let mut host = Host::new(window.hwnd(), width as u32, height as u32)?;
    host.ctx.set_pixels_per_point(scale);
    // 诊断：自研后端启动信息（窗口大小、缩放、后端名）。
    lingqi::resources::append_log(
        &lingqi::model::config_dir().join("diagnostics"),
        "renderer-self",
        &format!("灵栖自研渲染后端启动｜窗口 {}x{}｜缩放 {scale}｜适配器 {}", width, height, host.adapter_name()),
    );

    let mut app = App::new(&host.ctx, Options::default());
    app.migrate_merged_switches();
    let started = std::time::Instant::now();
    // 后台采样：电量、网络与输入法状态。
    let mut system_rx = lingqi::system::observe(host.ctx.clone(), lingqi::system::Snapshot::default(), None);
    let canvas = eframe::egui::Rect::from_min_size(
        eframe::egui::Pos2::ZERO,
        eframe::egui::vec2(app::CANVAS_W, app::WIN_H),
    );
    let mut env = Env::default();
    let mut repaint = std::time::Duration::from_millis(16);
    let mut frames: u64 = 0;
    let mut last_log = std::time::Instant::now();
    // 基准模式：不按节奏等待，连续出帧，用来量渲染器的真实吞吐。
    let bench_frames: u64 = std::env::var("LINGQI_SELF_BENCH")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0);
    let bench_started = std::time::Instant::now();
    render_loop::run(&mut host, canvas, |ctx| {
        frames += 1;
        // 每帧的运行环境：时间、电量、光标、锁屏。
        env.t = started.elapsed().as_secs_f64();
        let (clock, today, date) = local_now();
        env.clock = clock;
        env.today = today;
        env.date = date;
        env.screen_locked = lingqi::system::screen_locked();
        env.idle_seconds = Some(lingqi::system::idle_seconds());
        env.desktop_cursor = desktop_cursor();
        // 自研后端没有 egui 的窗口输入，光标位置要自己换算成窗口内坐标：
        // 岛体的悬停展开靠它。
        if let Some((desktop_x, desktop_y)) = env.desktop_cursor {
            env.cursor = Some(eframe::egui::Pos2::new(
                desktop_x as f32 / scale - origin_x,
                desktop_y as f32 / scale,
            ));
        } else {
            env.cursor = None;
        }
        while let Ok(observation) = system_rx.try_recv() {
            env.volume = observation.system.volume;
            env.network = Some(observation.system.network);
        }
        // 岛体画在自己的临时 Ui 里：egui 收集形状，交给自写渲染器呈现。
        let mut ui = eframe::egui::Ui::new(
            ctx.clone(),
            eframe::egui::Id::new(("lingqi-self", true)),
            eframe::egui::UiBuilder::new().max_rect(canvas),
        );
        let out = app.frame(&mut ui, canvas, &env);
        repaint = out.repaint_after;
        if bench_frames > 0 && frames >= bench_frames {
            let elapsed = bench_started.elapsed().as_secs_f64();
            lingqi::resources::append_log(
                &lingqi::model::config_dir().join("diagnostics"),
                "renderer-self",
                &format!(
                    "自研后端基准：{frames} 帧耗时 {elapsed:.2}s → {:.1} 帧/秒",
                    frames as f64 / elapsed.max(f64::EPSILON)
                ),
            );
            std::process::exit(0);
        }
        if last_log.elapsed().as_secs() >= 5 {
            lingqi::resources::append_log(
                &lingqi::model::config_dir().join("diagnostics"),
                "renderer-self",
                &format!("自研后端已出 {frames} 帧｜本次等待 {:?}", repaint),
            );
            frames = 0;
            last_log = std::time::Instant::now();
        }
        // 基准模式下不等待：连续出帧，量渲染器的真实吞吐。
        Some(if bench_frames > 0 { std::time::Duration::ZERO } else { repaint })
    })
}
fn run() -> eframe::Result<()> {
    // 自研渲染后端：显式要求时才用（LINGQI_RENDERER_BACKEND=self）。
    if lingqi::render::Backend::from_env().is_self_rendered() {
        match run_self_backend() {
            Ok(()) => return Ok(()),
            Err(error) => {
                diagnostics::report("自研渲染后端启动失败，已回退到 egui。", &error);
            }
        }
    }
    // 自动重启或「立即重启」拉起的新进程：先等旧进程退出，再做单实例检查。
    diagnostics::wait_for_previous_instance();
    // 单实例（与原版同名互斥体）
    #[cfg(windows)]
    if !profiling_island() && !lingqi::tray::single_instance() {
        if !lingqi::tray::activate_existing(false) {
            diagnostics::report("灵栖已在运行，但暂时无法打开窗口。\n请稍后重试；若仍无法打开，请在任务管理器的“详细信息”中结束 lingqi.exe 后重试。", "已有实例可能正在启动或已无响应。请稍后重试；若仍无法打开，请在任务管理器的“详细信息”中结束 lingqi.exe，再重新启动。");
        }
        return Ok(());
    }
    #[cfg(windows)]
    if !profiling_island(){std::thread::spawn(lingqi::record_install_size);}
    diagnostics::startup("初始化图形窗口");
    let viewport = egui::ViewportBuilder::default()
        .with_title("灵栖").with_icon(lingqi::theme::app_icon())
        .with_inner_size([app::CANVAS_W, app::WIN_H])
        .with_decorations(false)
        .with_transparent(true)
        .with_active(false)
        .with_resizable(false)
        .with_taskbar(false)
        .with_always_on_top()
        .with_visible(false);
    let opts = eframe::NativeOptions { viewport, vsync: !cfg!(windows), ..Default::default() };
    graphics::run(opts,|opts|{
        let result=eframe::run_native("灵栖",opts,Box::new(|cc|Ok(Box::new(Shell::new(cc)))));
        #[cfg(all(windows, feature = "material"))]
        if lingqi::material::report_run_failure(&result) {
            #[cfg(feature = "material")]
            if !lingqi::material::debug_requested() {
                use lingqi::sdk_spike::LocalPlugins;
                let mut plugins = LocalPlugins::open(lingqi::model::config_dir().join("local-plugins"));
                if let Err(error) = plugins.activate_material(None, false) {
                    lingqi::material::Runtime::report_failure(&format!("无法保存材质停用状态：{error}"));
                }
            }
            if replace_material_window(true) { return Ok(()); }
        }
        result
    })
}