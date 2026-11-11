//! Windows-specific system proxy plumbing: WinINet registry values (PAC URL), the
//! WinINet refresh notification, and `HKCU\Environment` helpers.
//! Compiled only on `target_os = "windows"` (see `sysproxy/mod.rs`).

use super::{enable_sysproxy, silent_command, LAST_PAC_URL};

/// Removes the PAC URL and disables the proxy flag in the WinINet registry values.
pub(super) fn disable_system_proxy() {
    // On Windows remove HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings\AutoConfigURL
    let _ = silent_command("reg")
        .args([
            "delete",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
            "/v",
            "AutoConfigURL",
            "/f",
        ])
        .output();
    let _ = silent_command("reg")
        .args([
            "add",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
            "/v",
            "ProxyEnable",
            "/t",
            "REG_DWORD",
            "/d",
            "0",
            "/f",
        ])
        .output();
    notify_windows_inet_refresh();
}

/// Writes the PAC URL into the WinINet registry values and records it in `configured`.
pub(super) fn enable_system_proxy(pac_url: &str, configured: &mut Vec<String>) {
    let output = silent_command("reg")
        .args([
            "add",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
            "/v",
            "AutoConfigURL",
            "/t",
            "REG_SZ",
            "/d",
            pac_url,
            "/f",
        ])
        .output();

    let _ = silent_command("reg")
        .args([
            "add",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
            "/v",
            "ProxyEnable",
            "/t",
            "REG_DWORD",
            "/d",
            "0",
            "/f",
        ])
        .output();

    if output.is_ok() {
        configured.push("Windows Internet Settings (WinINet)".to_string());
    }
    notify_windows_inet_refresh();
}

/// Windows branch of `refresh_if_services_changed`: re-applies the PAC URL when the registry value drifted.
pub(super) fn refresh_if_changed() -> bool {
    let pac = LAST_PAC_URL.lock().ok().and_then(|g| g.clone());
    if let Some(pac) = pac {
        if let Ok(output) = silent_command("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
                "/v",
                "AutoConfigURL",
            ])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !output.status.success() || !stdout.contains(&pac) {
                return enable_sysproxy(&pac).is_ok();
            }
        }
    }
    false
}

pub(super) fn notify_windows_inet_refresh() {
    #[link(name = "wininet")]
    unsafe extern "system" {
        fn InternetSetOptionW(
            hInternet: *mut core::ffi::c_void,
            dwOption: u32,
            lpBuffer: *mut core::ffi::c_void,
            dwBufferLength: u32,
        ) -> i32;
    }
    const INTERNET_OPTION_SETTINGS_CHANGED: u32 = 39;
    const INTERNET_OPTION_REFRESH: u32 = 37;
    unsafe {
        let _ = InternetSetOptionW(
            core::ptr::null_mut(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            core::ptr::null_mut(),
            0,
        );
        let _ = InternetSetOptionW(
            core::ptr::null_mut(),
            INTERNET_OPTION_REFRESH,
            core::ptr::null_mut(),
            0,
        );
    }
}

pub fn set_windows_user_env_var(name: &str, value: &str) -> std::io::Result<()> {
    silent_command("reg")
        .args(["add", r"HKCU\Environment", "/v", name, "/t", "REG_SZ", "/d", value, "/f"])
        .output()?;
    Ok(())
}

pub fn delete_windows_user_env_var(name: &str) -> std::io::Result<()> {
    let _ = silent_command("reg")
        .args(["delete", r"HKCU\Environment", "/v", name, "/f"])
        .output();
    Ok(())
}

pub fn broadcast_windows_env_change() {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SendMessageTimeoutW(
            hWnd: *mut core::ffi::c_void,
            Msg: u32,
            wParam: usize,
            lParam: *const u16,
            fuFlags: u32,
            uTimeout: u32,
            lpdwResult: *mut usize,
        ) -> isize;
        fn SendMessageTimeoutA(
            hWnd: *mut core::ffi::c_void,
            Msg: u32,
            wParam: usize,
            lParam: *const u8,
            fuFlags: u32,
            uTimeout: u32,
            lpdwResult: *mut usize,
        ) -> isize;
    }
    const HWND_BROADCAST: *mut core::ffi::c_void = 0xffff as *mut core::ffi::c_void;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;
    let env_wide: Vec<u16> = "Environment\0".encode_utf16().collect();
    let env_ansi: &[u8] = b"Environment\0";
    let mut dw_result: usize = 0;
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            env_wide.as_ptr(),
            SMTO_ABORTIFHUNG,
            2000,
            &mut dw_result,
        );
        let _ = SendMessageTimeoutA(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            env_ansi.as_ptr(),
            SMTO_ABORTIFHUNG,
            2000,
            &mut dw_result,
        );
    }
}
