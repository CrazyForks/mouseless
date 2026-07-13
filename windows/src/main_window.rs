use crate::{do_render_overlay, do_toggle_overlay, on_command, on_tray};
use std::sync::OnceLock;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, RegisterClassW, SW_HIDE, WM_COMMAND, WM_DESTROY,
    WNDCLASS_STYLES, WNDCLASSW,
};

const WM_TRAY_MSG: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 1;
pub const WM_TOGGLE_OVERLAY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 2;
pub const WM_RENDER_OVERLAY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 3;

fn class_name() -> windows::core::PCWSTR {
    static NAME: OnceLock<Vec<u16>> = OnceLock::new();
    let v = NAME.get_or_init(|| {
        "MouselessApp"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    });
    windows::core::PCWSTR::from_raw(v.as_ptr())
}

extern "system" fn main_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY_MSG => on_tray(hwnd, lparam),
        WM_COMMAND => {
            on_command((wparam.0 & 0xffff) as u32);
            LRESULT(0)
        }
        WM_TOGGLE_OVERLAY => {
            do_toggle_overlay();
            LRESULT(0)
        }
        WM_RENDER_OVERLAY => {
            do_render_overlay();
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn create_main_window() -> HWND {
    unsafe {
        let instance = HINSTANCE(
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .unwrap_or_default()
                .0,
        );
        let wc = WNDCLASSW {
            style: WNDCLASS_STYLES(0),
            lpfnWndProc: Some(main_wnd_proc),
            hInstance: instance,
            lpszClassName: class_name(),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);

        let hwnd = match CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            class_name(),
            windows::core::PCWSTR::null(),
            windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW,
            0,
            0,
            0,
            0,
            windows::Win32::UI::WindowsAndMessaging::HWND_MESSAGE,
            None,
            instance,
            None,
        ) {
            Ok(h) => h,
            Err(_) => HWND(std::ptr::null_mut()),
        };
        if !hwnd.is_invalid() {
            let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(hwnd, SW_HIDE);
        }
        hwnd
    }
}
