#![windows_subsystem = "windows"]

mod accessibility;
mod common;
mod core;
mod free_mode;
mod input;
mod main_window;
mod monitor;
mod mouse;
mod overlay;
mod preferences;
mod settings;
mod toast;
mod tray;

use crate::common::Rect;
use crate::core::{Environment, MouseBackend};
use crate::input::{current_modifiers, label_for_vk};
use crate::settings::{Settings, SettingsStore};
use std::sync::{Arc, Mutex, OnceLock};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegGetValueW, RegOpenKeyExW, RegSetValueExW, RRF_RT_REG_SZ,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PostMessageW, PostQuitMessage, TranslateMessage, DispatchMessageW, WM_LBUTTONDBLCLK,
    WM_LBUTTONUP, WM_RBUTTONUP,
};

struct Shared {
    settings: Arc<Mutex<Settings>>,
    store: Arc<Mutex<SettingsStore>>,
    state: Mutex<core::OverlayState>,
    windows: Mutex<overlay::OverlayWindows>,
    mouse: Arc<mouse::SendInputMouse>,
    free_mode: Mutex<free_mode::FreeModeController>,
    env: Arc<monitor::SystemEnvironment>,
    tray: Mutex<Option<tray::TrayIcon>>,
}

static SHARED: OnceLock<Shared> = OnceLock::new();
static MAIN_HWND: OnceLock<usize> = OnceLock::new();

fn post_main(msg: u32) {
    if let Some(&addr) = MAIN_HWND.get() {
        unsafe {
            let _ = PostMessageW(HWND(addr as *mut std::ffi::c_void), msg, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn do_toggle_overlay() {
    if let Some(shared) = SHARED.get() {
        toggle_overlay(shared);
    }
}

pub fn do_render_overlay() {
    if let Some(shared) = SHARED.get() {
        let state = shared.state.lock().unwrap();
        if state.is_visible() {
            let monitors = shared.env.monitors();
            let snap = state.snapshot();
            shared.windows.lock().unwrap().render(&snap, &monitors);
        }
    }
}

pub fn toggle_overlay(shared: &Shared) {
    let mut state = shared.state.lock().unwrap();
    let mut windows = shared.windows.lock().unwrap();
    if state.is_visible() {
        state.hide();
        windows.hide();
    } else {
        let monitors = shared.env.monitors();
        state.show();
        windows.show(&monitors);
        windows.render(&state.snapshot(), &monitors);
    }
}

fn dispatch_key(vk: u32, is_down: bool) -> i32 {
    let shared = match SHARED.get() {
        Some(s) => s,
        None => return 0,
    };

    let label = match label_for_vk(vk) {
        Some(l) => l,
        None => return 0,
    };
    let (ctrl, alt, shift, win) = current_modifiers();

    if is_down {
        let s = shared.settings.lock().unwrap();
        if s.overlay_hotkey.matches(&label, ctrl, alt, shift, win) {
            drop(s);
            // Defer the (expensive) window creation + render to the message
            // loop. Doing it here would exceed the low-level hook timeout and
            // get the hook silently disabled by Windows.
            post_main(main_window::WM_TOGGLE_OVERLAY);
            return 1;
        }
    }

    {
        let mut fm = shared.free_mode.lock().unwrap();
        if fm.is_active() {
            let s = shared.settings.lock().unwrap();
            let consumed = fm.handle(&label, &s);
            return if consumed { 1 } else { 0 };
        }
    }

    {
        let mut state = shared.state.lock().unwrap();
        if state.is_visible() {
            let hide = state.handle_key(&label, is_down);
            if hide {
                state.hide();
                // `hide()` only calls ShowWindow(SW_HIDE), which is cheap and
                // safe to run inside the hook. It also ensures the overlay is
                // gone before the async click thread scans for targets.
                shared.windows.lock().unwrap().hide();
            } else {
                // Rendering is expensive (full-screen per-pixel work); defer
                // it to the message loop to keep the hook callback fast.
                post_main(main_window::WM_RENDER_OVERLAY);
            }
            return 1;
        }
    }

    0
}

pub fn launch_enabled() -> bool {
    unsafe {
        let mut key = HKEY::default();
        let sub = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>();
        let name = "Mouseless"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR::from_raw(sub.as_ptr()),
            0,
            KEY_READ,
            &mut key,
        )
        .is_ok()
        {
            let mut buf = [0u16; 512];
            let mut size = (buf.len() * 2) as u32;
            let res = RegGetValueW(
                key,
                windows::core::PCWSTR::null(),
                windows::core::PCWSTR::from_raw(name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr() as *mut std::ffi::c_void),
                Some(&mut size),
            );
            let _ = RegCloseKey(key);
            res.is_ok() && size > 0
        } else {
            false
        }
    }
}

pub fn set_launch(enabled: bool) {
    unsafe {
        let mut key = HKEY::default();
        let path = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR::from_raw(path.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut key,
        )
        .is_ok()
        {
            let name = "Mouseless"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>();
            if enabled {
                let mut exe = [0u16; 512];
                let n = GetModuleFileNameW(None, &mut exe) as usize;
                let bytes = std::slice::from_raw_parts(
                    exe.as_ptr() as *const u8,
                    (n + 1) * 2,
                );
                let _ = RegSetValueExW(
                    key,
                    windows::core::PCWSTR::from_raw(name.as_ptr()),
                    0,
                    REG_SZ,
                    Some(bytes),
                );
            } else {
                let _ = RegDeleteValueW(key, windows::core::PCWSTR::from_raw(name.as_ptr()));
            }
            let _ = RegCloseKey(key);
        }
    }
}

pub fn on_tray(hwnd: HWND, lparam: windows::Win32::Foundation::LPARAM) -> windows::Win32::Foundation::LRESULT {
    let event = (lparam.0 as u32) & 0xffff;
    if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
        tray::show_tray_menu(hwnd, launch_enabled());
    } else if event == WM_LBUTTONDBLCLK {
        if let Some(s) = SHARED.get() {
            toggle_overlay(s);
        }
    }
    windows::Win32::Foundation::LRESULT(0)
}

pub fn on_command(id: u32) {
    match id {
        tray::ID_SHOW_OVERLAY => {
            if let Some(s) = SHARED.get() {
                toggle_overlay(s);
            }
        }
        tray::ID_TOGGLE_FREE_MODE => {
            if let Some(s) = SHARED.get() {
                s.free_mode.lock().unwrap().toggle();
            }
        }
        tray::ID_PREFERENCES => open_preferences_flow(),
        tray::ID_LAUNCH_STARTUP => {
            let cur = launch_enabled();
            set_launch(!cur);
        }
        tray::ID_QUIT => unsafe {
            PostQuitMessage(0);
        },
        _ => {}
    }
}

fn open_preferences_flow() {
    if let Some(shared) = SHARED.get() {
        let ctx = preferences::PrefsContext {
            settings: shared.settings.clone(),
            on_apply: Arc::new(|| {
                if let Some(s) = SHARED.get() {
                    let st = s.settings.lock().unwrap();
                    s.store.lock().unwrap().settings = st.clone();
                    s.store.lock().unwrap().save();
                    let state = s.state.lock().unwrap();
                    if state.is_visible() {
                        let monitors = s.env.monitors();
                        let snap = state.snapshot();
                        s.windows.lock().unwrap().render(&snap, &monitors);
                    }
                }
            }),
            launch_get: Arc::new(launch_enabled),
            launch_set: Arc::new(set_launch),
        };
        preferences::open_preferences(ctx);
    }
}

fn main() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let store = SettingsStore::new();
    let settings = Arc::new(Mutex::new(store.settings.clone()));
    let store = Arc::new(Mutex::new(store));
    let mouse = Arc::new(mouse::SendInputMouse::new());
    let detector = Arc::new(accessibility::UiAutomationDetector::new());
    let env = Arc::new(monitor::SystemEnvironment::new());

    let state = core::OverlayState::new(settings.clone(), mouse.clone(), detector, env.clone());
    let windows = overlay::OverlayWindows::new();

    let notify: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(|text: &str| toast::show_toast(text));
    let free_mode = free_mode::FreeModeController::new(mouse.clone(), env.clone(), notify);

    let shared = Shared {
        settings: settings.clone(),
        store: store.clone(),
        state: Mutex::new(state),
        windows: Mutex::new(windows),
        mouse: mouse.clone(),
        free_mode: Mutex::new(free_mode),
        env: env.clone(),
        tray: Mutex::new(None),
    };
    let _ = SHARED.set(shared);

    let hwnd = main_window::create_main_window();
    let _ = MAIN_HWND.set(hwnd.0 as usize);
    preferences::set_prefs_owner(hwnd);
    {
        let tray_icon = tray::setup_tray(hwnd);
        *SHARED.get().unwrap().tray.lock().unwrap() = Some(tray_icon);
    }
    toast::show_toast("Mouseless ready");

    let _ = input::install_keyboard_hook(Box::new(|vk, down| dispatch_key(vk, down)));

    unsafe {
        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    input::uninstall_keyboard_hook();
    if let Some(t) = SHARED.get().unwrap().tray.lock().unwrap().take() {
        tray::remove_tray(&t);
    }
}

#[allow(dead_code)]
fn _keep_traits() {
    fn _assert<T: Environment + MouseBackend>() {}
    let _ = Rect::new(0.0, 0.0, 1.0, 1.0);
}
