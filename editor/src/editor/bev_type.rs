//! Double-clicking a .bev file opens Beverly: the file type is registered for this Windows user (HKEY_CURRENT_USER,
//! no admin needed) the first time Beverly runs, and again when it runs from somewhere new.

#[cfg(windows)]
pub fn register() {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegGetValueW, RegSetKeyValueW};
    use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.display().to_string();
    let open = format!("\"{exe}\" \"%1\"");
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let key = |k: &str| wide(&format!(r"Software\Classes\{k}"));
    // already opening with this Beverly?
    let mut buf = [0u16; 1024];
    let mut bytes = std::mem::size_of_val(&buf) as u32;
    let read = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, key(r"Beverly.Project\shell\open\command").as_ptr(), std::ptr::null(), RRF_RT_REG_SZ,
            std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut bytes)
    };
    if read == 0 && String::from_utf16_lossy(&buf[..(bytes as usize / 2).saturating_sub(1)]) == open { return; }
    let set = |k: &str, v: &str| {
        let (k, v) = (key(k), wide(v));
        unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, k.as_ptr(), std::ptr::null(), REG_SZ, v.as_ptr().cast(), (v.len() * 2) as u32) };
    };
    set(".bev", "Beverly.Project");
    set("Beverly.Project", "Beverly project");
    set(r"Beverly.Project\DefaultIcon", &format!("\"{exe}\",0"));
    set(r"Beverly.Project\shell\open\command", &open);
    // so Explorer shows the new icon without a restart
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED as _, SHCNF_IDLIST, std::ptr::null(), std::ptr::null()) };
}

#[cfg(not(windows))]
pub fn register() {}
