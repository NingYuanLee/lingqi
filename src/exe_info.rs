//! 进程与可执行文件的通用查询：读映像路径、核对身份、读文件版本号。
//!
//! 原先这些函数住在音乐模块里，但消息提醒窗口重启、诊断面板都要用，
//! 与音乐无关，因此单独成模块。
use windows::Win32::{
    Foundation::CloseHandle,
    System::{
        Diagnostics::ToolHelp::{CreateToolhelp32Snapshot,Process32FirstW,Process32NextW,PROCESSENTRY32W,TH32CS_SNAPPROCESS},
        Threading::{OpenProcess,QueryFullProcessImageNameW,PROCESS_NAME_WIN32,PROCESS_QUERY_LIMITED_INFORMATION},
    },
};

fn exe_name(path: &str) -> &str { path.rsplit(['\\','/']).next().unwrap_or(path) }

/// 从进程快照按可执行文件名筛选进程号；快照不需要打开任何进程。
pub fn pids_named(exe: &str) -> Vec<u32> {
    let mut pids = Vec::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return pids };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snapshot, &mut entry).is_ok();
        while ok {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case(exe) { pids.push(entry.th32ProcessID); }
            ok = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    pids
}

/// 仅以有限查询权限核对进程映像名（进程号可能被复用，每次使用前都要核对）。
pub fn image_is(pid: u32, exe: &str) -> bool {
    image_path(pid).is_some_and(|path| exe_name(&path).eq_ignore_ascii_case(exe))
}

/// 用「查询有限信息」权限读取进程的可执行文件路径；只用于核对身份和读取版本号。
pub fn image_path(pid: u32) -> Option<String> {
    if pid == 0 { return None; }
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut path = [0u16; 1024];
        let mut length = path.len() as u32;
        let result = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, windows::core::PWSTR(path.as_mut_ptr()), &mut length);
        let _ = CloseHandle(process);
        result.ok()?;
        Some(String::from_utf16_lossy(&path[..length as usize]))
    }
}

/// 读取可执行文件的版本号，取不到时返回「未知」。
pub(crate) fn file_version(path: &str) -> String {
    use windows::{core::{HSTRING,w},Win32::Storage::FileSystem::{GetFileVersionInfoSizeW,GetFileVersionInfoW,VerQueryValueW,VS_FIXEDFILEINFO}};
    let path = HSTRING::from(path);
    unsafe {
        let size = GetFileVersionInfoSizeW(&path, None);
        if size == 0 || size > 1 << 20 { return "未知".into(); }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(&path, 0, size, data.as_mut_ptr().cast()).is_err() { return "未知".into(); }
        let mut info = std::ptr::null_mut();
        let mut length = 0;
        if !VerQueryValueW(data.as_ptr().cast(), w!("\\"), &mut info, &mut length).as_bool() || info.is_null()
            || (length as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>() { return "未知".into(); }
        let info = &*(info as *const VS_FIXEDFILEINFO);
        format!("{}.{}.{}.{}", info.dwFileVersionMS >> 16, info.dwFileVersionMS & 0xffff, info.dwFileVersionLS >> 16, info.dwFileVersionLS & 0xffff)
    }
}

#[test]
fn exe_name_takes_file_part(){
    assert_eq!(exe_name(r"C:\Program Files\KuGou\KuGou.exe"),"KuGou.exe");
    assert_eq!(exe_name("cloudmusic.exe"),"cloudmusic.exe");
}

#[test]
fn snapshot_finds_own_process(){
    let me=std::env::current_exe().unwrap();
    let exe=me.file_name().unwrap().to_str().unwrap();
    let pid=std::process::id();
    assert!(pids_named(exe).contains(&pid));
    assert!(image_is(pid,exe));
    assert!(!image_is(pid,"KuGou.exe"));
}
