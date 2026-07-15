use std::sync::OnceLock;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, LoadIconW, SetForegroundWindow,
    TrackPopupMenu, MF_CHECKED, MF_STRING, MF_UNCHECKED, TPM_BOTTOMALIGN, TPM_LEFTALIGN, WM_USER,
};

pub const WM_TRAY: u32 = WM_USER + 1;
pub const ID_SHOW_OVERLAY: u32 = 1001;
pub const ID_TOGGLE_FREE_MODE: u32 = 1002;
pub const ID_PREFERENCES: u32 = 1003;
pub const ID_LAUNCH_STARTUP: u32 = 1004;
pub const ID_QUIT: u32 = 1005;

fn tip() -> [u16; 128] {
    let mut buf = [0u16; 128];
    let wide: Vec<u16> = "Mouseless".encode_utf16().collect();
    for (i, c) in wide.iter().take(127).enumerate() {
        buf[i] = *c;
    }
    buf
}

/// Resource ID of the embedded application icon (see resources/resource.rc).
const APP_ICON_ID: u32 = 1;

/// Loads the embedded application icon from the executable's resources.
/// Falls back to the default Windows application icon if the resource is
/// missing (e.g. when built without `build.rs` resource embedding).
pub fn load_app_icon() -> windows::Win32::UI::WindowsAndMessaging::HICON {
    let instance = unsafe {
        HINSTANCE(
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .unwrap_or_default()
                .0,
        )
    };
    // MAKEINTRESOURCEW(id) — pack the integer resource ID into a PCWSTR.
    let name = PCWSTR(APP_ICON_ID as usize as *const u16);
    unsafe { LoadIconW(instance, name).unwrap_or_default() }
}

pub struct TrayIcon {
    pub hwnd: HWND,
    pub icon_data: NOTIFYICONDATAW,
}

unsafe impl Send for TrayIcon {}
unsafe impl Sync for TrayIcon {}

pub fn setup_tray(hwnd: HWND) -> TrayIcon {
    let icon = load_app_icon();
    let icon_data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        hIcon: icon,
        szTip: tip(),
        ..Default::default()
    };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &icon_data);
    }
    TrayIcon { hwnd, icon_data }
}

pub fn remove_tray(icon: &TrayIcon) {
    unsafe {
        let data = icon.icon_data;
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

pub fn update_tray(icon: &TrayIcon) {
    unsafe {
        let data = icon.icon_data;
        let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
    }
}

pub fn show_tray_menu(hwnd: HWND, launch_enabled: bool) {
    unsafe {
        let menu = match CreatePopupMenu() {
            Ok(m) => m,
            Err(_) => return,
        };
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            ID_SHOW_OVERLAY as usize,
            windows::core::PCWSTR::from_raw(to_wide("Show Overlay").as_ptr()),
        );
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            ID_TOGGLE_FREE_MODE as usize,
            windows::core::PCWSTR::from_raw(to_wide("Toggle Free Mode").as_ptr()),
        );
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            ID_PREFERENCES as usize,
            windows::core::PCWSTR::from_raw(to_wide("Preferences...").as_ptr()),
        );
        let launch_label = if launch_enabled {
            "Launch at Startup (on)"
        } else {
            "Launch at Startup (off)"
        };
        let launch_flags = if launch_enabled {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING | MF_UNCHECKED
        };
        let _ = AppendMenuW(
            menu,
            launch_flags,
            ID_LAUNCH_STARTUP as usize,
            windows::core::PCWSTR::from_raw(to_wide(launch_label).as_ptr()),
        );
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            ID_QUIT as usize,
            windows::core::PCWSTR::from_raw(to_wide("Quit Mouseless").as_ptr()),
        );

        let mut pt = windows::Win32::Foundation::POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
