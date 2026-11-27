use std::{collections::BTreeMap, mem::size_of};

use serde_json::{json, Map, Value};
use windows::{
    core::{w, HSTRING, PCWSTR},
    Win32::{
        Foundation::{CloseHandle, BOOL, FILETIME, HWND, LPARAM, RECT},
        Graphics::Gdi::{EnumDisplayDevicesW, DISPLAY_DEVICEW},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            Registry::HKEY_LOCAL_MACHINE,
            SystemInformation::GetTickCount64,
            Threading::{
                GetGuiResources, GetProcessHandleCount, GetProcessTimes, OpenProcess, GR_GDIOBJECTS,
                GR_USEROBJECTS, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        UI::WindowsAndMessaging::*,
    },
};
use windows_sys::Win32::System::{
    EventLog::*,
    ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
};

use crate::exe_info;
use windows::Win32::System::Registry::{HKEY, RegGetValueW, RRF_RT_REG_SZ};

/// 读注册表里的字符串值（安装路径之类）。
pub(crate) fn registry_string(root:HKEY,key:PCWSTR,name:PCWSTR)->Option<String>{
    let mut data=[0u16;2048];let mut size=(data.len()*2) as u32;
    unsafe {RegGetValueW(root,key,name,RRF_RT_REG_SZ,None,Some(data.as_mut_ptr().cast()),Some(&mut size))}.ok().ok()?;
    let length=data.iter().position(|c|*c==0).unwrap_or(data.len());
    Some(String::from_utf16_lossy(&data[..length]))
}

/// 可执行文件的厂商名（从版本信息里取）。
pub(crate) fn company_name(path:&str)->String{
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW,GetFileVersionInfoW,VerQueryValueW};
    static CACHE:std::sync::Mutex<Option<std::collections::HashMap<String,String>>>=std::sync::Mutex::new(None);
    if let Some(name)=CACHE.lock().ok().and_then(|c|c.as_ref().and_then(|c|c.get(path).cloned())){return name;}
    let name=unsafe{(||->Option<String>{
        let wide:Vec<u16>=path.encode_utf16().chain(Some(0)).collect();
        let size=GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()),None);
        if size==0 {return None;}
        let mut data=vec![0u8;size as usize];
        GetFileVersionInfoW(PCWSTR(wide.as_ptr()),0,size,data.as_mut_ptr().cast()).ok()?;
        let mut pointer=std::ptr::null_mut();let mut length=0u32;
        if !VerQueryValueW(data.as_ptr().cast(),w!("\\VarFileInfo\\Translation"),&mut pointer,&mut length).as_bool() || length<4 {return None;}
        let translation=*(pointer as *const [u16;2]);
        let key:Vec<u16>=format!("\\StringFileInfo\\{:04x}{:04x}\\CompanyName",translation[0],translation[1]).encode_utf16().chain(Some(0)).collect();
        if !VerQueryValueW(data.as_ptr().cast(),PCWSTR(key.as_ptr()),&mut pointer,&mut length).as_bool() || length==0 {return None;}
        let text=std::slice::from_raw_parts(pointer as *const u16,length as usize);
        Some(String::from_utf16_lossy(text).trim_end_matches('\0').to_string())
    })()}.unwrap_or_default();
    if let Ok(mut cache)=CACHE.lock(){cache.get_or_insert_with(Default::default).insert(path.to_string(),name.clone());}
    name
}
const MESSENGERS: &[&str] = &["weixin", "wechat", "wxwork", "feishu", "dingtalk", "qq"];
const PLAYERS: &[&str] = &["cloudmusic", "qqmusic", "kugou", "sodamusic", "kwmusic", "bodian_pc", "spotify", "applemusic"];

pub(super) fn collect() -> Map<String, Value> {
    let mut info = Map::new();
    let key = w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let system = |name| registry_string(HKEY_LOCAL_MACHINE, key, name);
    let build = system(w!("CurrentBuildNumber"));
    let name = system(w!("ProductName")).map(|name| {
        // Windows 11 的 ProductName 可能仍沿用 Windows 10 字样。
        if build.as_ref().and_then(|build| build.parse::<u32>().ok()).is_some_and(|build| build >= 22000) {
            name.replacen("Windows 10", "Windows 11", 1)
        } else {
            name
        }
    });
    let uptime = chrono::TimeDelta::milliseconds(unsafe { GetTickCount64() } as i64);
    info.insert("System".into(), json!({
        "Name": name,
        "Version": system(w!("DisplayVersion")),
        "Build": build,
        "BootedAt": (chrono::Utc::now() - uptime).to_rfc3339(),
    }));
    info.insert("Graphics".into(), json!(graphics()));
    let screens: Vec<_> = crate::display_layout::connected().into_iter().map(|screen| json!({
        "Device": screen.label, "Primary": screen.primary, "X": screen.bounds.left,
        "Y": screen.bounds.top, "Width": screen.bounds.right - screen.bounds.left,
        "Height": screen.bounds.bottom - screen.bounds.top,
    })).collect();
    info.insert("Screens".into(), json!(screens));
    process_details(&mut info);
    recent_events(&mut info);
    info
}

fn utf16(text: &[u16]) -> String {
    String::from_utf16_lossy(&text[..text.iter().position(|&c| c == 0).unwrap_or(text.len())])
}

fn graphics() -> Vec<Value> {
    let mut rows = Vec::new();
    for index in 0..64 {
        let mut device = DISPLAY_DEVICEW { cb: size_of::<DISPLAY_DEVICEW>() as u32, ..Default::default() };
        if !unsafe { EnumDisplayDevicesW(None, index, &mut device, 0) }.as_bool() {
            break;
        }
        let path = utf16(&device.DeviceKey);
        let driver = |name| {
            let key = HSTRING::from(path.strip_prefix(r"\Registry\Machine\")?);
            registry_string(HKEY_LOCAL_MACHINE, PCWSTR(key.as_ptr()), name)
        };
        let row = json!({"Name": utf16(&device.DeviceString),
            "Driver": driver(w!("DriverVersion")), "DriverDate": driver(w!("DriverDate"))});
        if !rows.contains(&row) {
            rows.push(row);
        }
    }
    rows
}

fn process_details(info: &mut Map<String, Value>) {
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        info.insert("ProcessQuery".into(), json!("无法读取进程列表"));
        return;
    };
    let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut island = Vec::new();
    let mut messengers = Vec::new();
    let mut messenger_ids = Vec::new();
    let mut players: BTreeMap<String, (u32, Option<String>)> = BTreeMap::new();
    unsafe {
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            let exe = utf16(&entry.szExeFile).to_ascii_lowercase();
            let name = exe.strip_suffix(".exe").unwrap_or(&exe);
            let pid = entry.th32ProcessID;
            // 核对快照中的 PID 仍属于目标应用，仅查询文件版本与进程计数。
            let path = || exe_info::image_path(pid)
                .filter(|path| path.rsplit(['\\', '/']).next().is_some_and(|file| file.eq_ignore_ascii_case(&exe)));
            if name == "lingqi" {
                if let Some(path) = path() {
                    island.push(island_process(pid, entry.cntThreads, &path));
                }
            } else if MESSENGERS.contains(&name) {
                if let Some(path) = path() {
                    messenger_ids.push(pid);
                    messengers.push(json!({"Name": name, "Id": pid, "Parent": entry.th32ParentProcessID,
                        "Company": company_name(&path), "Version": exe_info::file_version(&path)}));
                }
            } else if PLAYERS.contains(&name) {
                let player = players.entry(name.to_owned()).or_default();
                player.0 += 1;
                if player.1.is_none() {
                    player.1 = path().map(|p| exe_info::file_version(&p));
                }
            }
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    let players: Vec<_> = players.into_iter().map(|(name, (count, version))|
        json!({"Name": name, "Processes": count, "Version": version})).collect();
    info.insert("MusicPlayers".into(), json!(players));
    info.insert("Processes".into(), json!(island));
    info.insert("MessengerProcesses".into(), json!(messengers));
    let mut windows = (messenger_ids, Vec::<String>::new());
    unsafe { let _ = EnumWindows(Some(messenger_window), LPARAM((&mut windows as *mut _) as isize)); }
    info.insert("MessengerWindows".into(), json!(windows.1));
}

fn filetime(ticks: u64) -> Option<String> {
    let seconds = i64::try_from(ticks / 10_000_000).ok()? - 11_644_473_600;
    let nanos = (ticks % 10_000_000) as u32 * 100;
    chrono::DateTime::from_timestamp(seconds, nanos).map(|value| value.to_rfc3339())
}

fn island_process(pid: u32, threads: u32, path: &str) -> Value {
    let mut row = json!({"Id": pid, "Threads": threads, "Version": exe_info::file_version(path)});
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return row };
        let mut started = FILETIME::default();
        if GetProcessTimes(process, &mut started, &mut FILETIME::default(),
            &mut FILETIME::default(), &mut FILETIME::default()).is_ok() {
            row["StartedAt"] = json!(filetime(((started.dwHighDateTime as u64) << 32) | started.dwLowDateTime as u64));
        }
        let mut handles = 0;
        if GetProcessHandleCount(process, &mut handles).is_ok() {
            row["Handles"] = json!(handles);
        }
        row["GdiObjects"] = json!(GetGuiResources(process, GR_GDIOBJECTS));
        row["UserObjects"] = json!(GetGuiResources(process, GR_USEROBJECTS));
        let mut memory = PROCESS_MEMORY_COUNTERS_EX { cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32, ..Default::default() };
        if K32GetProcessMemoryInfo(process.0, (&mut memory as *mut PROCESS_MEMORY_COUNTERS_EX).cast(), memory.cb) != 0 {
            row["PrivateBytes"] = json!(memory.PrivateUsage);
            row["WorkingBytes"] = json!(memory.WorkingSetSize);
        }
        let _ = CloseHandle(process);
    }
    row
}

unsafe extern "system" fn messenger_window(window: HWND, data: LPARAM) -> BOOL {
    let (pids, rows) = &mut *(data.0 as *mut (Vec<u32>, Vec<String>));
    if rows.len() >= 400 {
        return BOOL(0);
    }
    let mut pid = 0;
    GetWindowThreadProcessId(window, Some(&mut pid));
    if pids.contains(&pid) {
        let mut class = [0u16; 128];
        let count = GetClassNameW(window, &mut class).max(0) as usize;
        let mut rect = RECT::default();
        let _ = GetWindowRect(window, &mut rect);
        rows.push(format!(
            "pid={pid} window={:#x} class={} visible={} minimized={} owned={} style={:#010x} exstyle={:#010x} size={}x{}",
            window.0 as usize, String::from_utf16_lossy(&class[..count]), IsWindowVisible(window).as_bool() as u8,
            IsIconic(window).as_bool() as u8, GetWindow(window, GW_OWNER).is_ok() as u8,
            GetWindowLongW(window, GWL_STYLE) as u32, GetWindowLongW(window, GWL_EXSTYLE) as u32,
            rect.right - rect.left, rect.bottom - rect.top,
        ));
    }
    BOOL(1)
}

struct EventHandle(EVT_HANDLE);

impl Drop for EventHandle {
    fn drop(&mut self) { unsafe { let _ = EvtClose(self.0); } }
}

fn recent_events(info: &mut Map<String, Value>) {
    let faults = events(w!("Application"), w!("*[System[(EventID=1000 or EventID=1001 or EventID=1002) and TimeCreated[timediff(@SystemTime) <= 604800000]] and EventData[Data[@Name='AppName']='lingqi.exe' or Data[@Name='P1']='lingqi.exe']]"), 30);
    let resets = events(w!("System"), w!("*[System[EventID=4101 and TimeCreated[timediff(@SystemTime) <= 604800000]]]"), 20);
    if faults.is_none() || resets.is_none() {
        info.insert("EventQuery".into(), json!("无法读取部分 Windows 事件记录"));
    }
    info.insert("ApplicationFaults".into(), json!(faults.unwrap_or_default()));
    info.insert("DisplayResets".into(), json!(resets.unwrap_or_default()));
}

fn events(channel: PCWSTR, query: PCWSTR, limit: usize) -> Option<Vec<Value>> {
    let fields = [
        ("EventId", w!("Event/System/EventID")),
        ("At", w!("Event/System/TimeCreated/@SystemTime")),
        ("Provider", w!("Event/System/Provider/@Name")),
        ("AppVersion", w!("Event/EventData/Data[@Name='AppVersion']")),
        ("ModuleName", w!("Event/EventData/Data[@Name='ModuleName']")),
        ("ModuleVersion", w!("Event/EventData/Data[@Name='ModuleVersion']")),
        ("ExceptionCode", w!("Event/EventData/Data[@Name='ExceptionCode']")),
        ("FaultingOffset", w!("Event/EventData/Data[@Name='FaultingOffset']")),
        ("EventName", w!("Event/EventData/Data[@Name='EventName']")),
    ];
    let paths = fields.map(|(_, path)| path.as_ptr());
    unsafe {
        let set = EvtQuery(0, channel.as_ptr(), query.as_ptr(), EvtQueryChannelPath | EvtQueryReverseDirection);
        if set == 0 { return None; }
        let set = EventHandle(set);
        let context = EvtCreateRenderContext(paths.len() as u32, paths.as_ptr(), EvtRenderContextValues);
        if context == 0 { return None; }
        let context = EventHandle(context);
        let mut rows = Vec::new();
        for _ in 0..limit {
            let mut event = [0isize];
            let mut found = 0;
            if EvtNext(set.0, 1, event.as_mut_ptr(), 500, 0, &mut found) == 0 || found == 0 {
                break;
            }
            let event = EventHandle(event[0]);
            let (mut bytes, mut count) = (0, 0);
            let flags = EvtRenderEventValues;
            let _ = EvtRender(context.0, event.0, flags, 0, std::ptr::null_mut(), &mut bytes, &mut count);
            if !(1..=65_536).contains(&bytes) {
                continue;
            }
            let mut values = vec![EVT_VARIANT::default(); (bytes as usize).div_ceil(size_of::<EVT_VARIANT>())];
            if EvtRender(context.0, event.0, flags, bytes, values.as_mut_ptr().cast(), &mut bytes, &mut count) == 0 {
                continue;
            }
            let mut row = Map::new();
            for ((name, _), value) in fields.iter().zip(values.iter().take(count as usize)) {
                let kind = value.Type as i32;
                let cell = if kind == EvtVarTypeUInt16 {
                    json!(value.Anonymous.UInt16Val)
                } else if kind == EvtVarTypeFileTime {
                    json!(filetime(value.Anonymous.FileTimeVal))
                } else if kind == EvtVarTypeString && !value.Anonymous.StringVal.is_null() {
                    json!(PCWSTR(value.Anonymous.StringVal).to_string().ok())
                } else {
                    continue
                };
                row.insert((*name).into(), cell);
            }
            rows.push(Value::Object(row));
        }
        Some(rows)
    }
}
