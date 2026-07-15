use crate::settings::{Hotkey, Settings};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::SystemServices::SS_LEFT;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetWindowLongPtrW, GetWindowTextW, SendMessageW, SetWindowLongPtrW,
    SetWindowTextW, ShowWindow, BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_PUSHBUTTON,
    ES_AUTOHSCROLL, ES_NUMBER, GWLP_USERDATA, SW_SHOW, WINDOW_STYLE, WS_BORDER, WS_CHILD,
    WS_VISIBLE,
};

pub const IDC_ROWS: u32 = 2001;
pub const IDC_COLS: u32 = 2002;
pub const IDC_OPACITY: u32 = 2003;
pub const IDC_STEP: u32 = 2004;
pub const IDC_CONTINUOUS: u32 = 2005;
pub const IDC_LAUNCH: u32 = 2006;
pub const IDC_HOTKEY_KEY: u32 = 2007;
pub const IDC_HOTKEY_CTRL: u32 = 2008;
pub const IDC_HOTKEY_ALT: u32 = 2009;
pub const IDC_HOTKEY_SHIFT: u32 = 2010;
pub const IDC_HOTKEY_WIN: u32 = 2011;
pub const IDC_QUITKEY: u32 = 2012;
pub const IDC_DONE: u32 = 2013;

pub struct PrefsContext {
    pub settings: Arc<Mutex<Settings>>,
    pub on_apply: Arc<dyn Fn() + Send + Sync>,
    pub launch_get: Arc<dyn Fn() -> bool + Send + Sync>,
    pub launch_set: Arc<dyn Fn(bool) + Send + Sync>,
}

struct Controls {
    rows: HWND,
    cols: HWND,
    opacity: HWND,
    step: HWND,
    continuous: HWND,
    launch: HWND,
    hotkey_key: HWND,
    hotkey_ctrl: HWND,
    hotkey_alt: HWND,
    hotkey_shift: HWND,
    hotkey_win: HWND,
    quitkey: HWND,
}

struct PrefsState {
    ctx: PrefsContext,
    ctrl: Controls,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn class(s: &str) -> windows::core::PCWSTR {
    windows::core::PCWSTR::from_raw(wide(s).as_ptr())
}

fn combo(parts: &[u32]) -> WINDOW_STYLE {
    let mut v = 0u32;
    for p in parts {
        v |= *p;
    }
    WINDOW_STYLE(v)
}

unsafe fn create_control(
    parent: HWND,
    class_name: &str,
    text: &str,
    id: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    style: WINDOW_STYLE,
    instance: windows::Win32::Foundation::HINSTANCE,
) -> HWND {
    windows::Win32::UI::WindowsAndMessaging::CreateWindowExW(
        windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
        class(class_name),
        windows::core::PCWSTR::from_raw(wide(text).as_ptr()),
        combo(&[WS_CHILD.0, WS_VISIBLE.0, style.0]),
        x,
        y,
        w,
        h,
        parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(id as usize as *mut std::ffi::c_void),
        instance,
        None,
    )
    .unwrap_or(HWND(std::ptr::null_mut()))
}

unsafe fn get_text(hwnd: HWND) -> String {
    let mut buf = [0u16; 64];
    let len = GetWindowTextW(hwnd, &mut buf);
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize])
        .trim()
        .to_string()
}

unsafe fn set_text(hwnd: HWND, text: &str) {
    let _ = SetWindowTextW(hwnd, windows::core::PCWSTR::from_raw(wide(text).as_ptr()));
}

unsafe fn is_checked(hwnd: HWND) -> bool {
    (SendMessageW(hwnd, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 & 1) != 0
}

unsafe fn set_checked(hwnd: HWND, checked: bool) {
    let _ = SendMessageW(
        hwnd,
        BM_SETCHECK,
        WPARAM(if checked { 1 } else { 0 }),
        LPARAM(0),
    );
}

extern "system" fn prefs_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == windows::Win32::UI::WindowsAndMessaging::WM_COMMAND {
            let id = (wparam.0 & 0xffff) as u32;
            if id == IDC_DONE {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PrefsState;
                if !ptr.is_null() {
                    let state = &*ptr;
                    apply(state);
                }
                let _ = DestroyWindow(hwnd);
                return LRESULT(0);
            } else if id == IDC_LAUNCH {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PrefsState;
                if !ptr.is_null() {
                    let checked = is_checked((*ptr).ctrl.launch);
                    ((*ptr).ctx.launch_set)(checked);
                }
                return LRESULT(0);
            }
        } else if msg == windows::Win32::UI::WindowsAndMessaging::WM_DESTROY {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PrefsState;
            if !ptr.is_null() {
                let _ = Box::from_raw(ptr);
                windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            return LRESULT(0);
        }
        windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

unsafe fn apply(state: &PrefsState) {
    let mut s = state.ctx.settings.lock().unwrap();
    if let Ok(v) = get_text(state.ctrl.rows).parse::<i32>() {
        s.grid_rows = v.clamp(3, 5);
    }
    if let Ok(v) = get_text(state.ctrl.cols).parse::<i32>() {
        s.grid_columns = v.clamp(3, 5);
    }
    if let Ok(v) = get_text(state.ctrl.opacity).parse::<f64>() {
        s.overlay_opacity = v.clamp(0.25, 0.95);
    }
    if let Ok(v) = get_text(state.ctrl.step).parse::<f64>() {
        s.free_mode_step = v.clamp(6.0, 90.0);
    }
    s.continuous_mode = is_checked(state.ctrl.continuous);

    let key = get_text(state.ctrl.hotkey_key);
    let hotkey = Hotkey::from_input(
        &key,
        is_checked(state.ctrl.hotkey_ctrl),
        is_checked(state.ctrl.hotkey_alt),
        is_checked(state.ctrl.hotkey_shift),
        is_checked(state.ctrl.hotkey_win),
    );
    if let Some(h) = hotkey {
        s.overlay_hotkey = h;
    }
    s.quit_grid_key = Hotkey::normalized_key(&get_text(state.ctrl.quitkey));
    drop(s);
    (state.ctx.on_apply)();
}

pub fn set_prefs_owner(_hwnd: HWND) {}

pub fn open_preferences(ctx: PrefsContext) {
    unsafe {
        let instance = windows::Win32::Foundation::HINSTANCE(
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .unwrap_or_default()
                .0,
        );
        static mut REGISTERED: bool = false;
        if !REGISTERED {
            let wc = windows::Win32::UI::WindowsAndMessaging::WNDCLASSW {
                style: windows::Win32::UI::WindowsAndMessaging::WNDCLASS_STYLES(0),
                lpfnWndProc: Some(prefs_wnd_proc),
                hInstance: instance,
                hIcon: crate::tray::load_app_icon(),
                lpszClassName: class("MouselessPrefs"),
                ..Default::default()
            };
            let _ = windows::Win32::UI::WindowsAndMessaging::RegisterClassW(&wc);
            REGISTERED = true;
        }

        let s = ctx.settings.lock().unwrap().clone();

        let hwnd = windows::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WS_EX_DLGMODALFRAME,
            class("MouselessPrefs"),
            windows::core::PCWSTR::from_raw(wide("Mouseless Preferences").as_ptr()),
            combo(&[
                WS_VISIBLE.0,
                WS_BORDER.0,
                windows::Win32::UI::WindowsAndMessaging::WS_CAPTION.0,
                windows::Win32::UI::WindowsAndMessaging::WS_SYSMENU.0,
            ]),
            120,
            120,
            420,
            430,
            None,
            None,
            instance,
            None,
        )
        .unwrap_or(HWND(std::ptr::null_mut()));
        if hwnd.is_invalid() {
            return;
        }

        let mk_label = |text: &str, x: i32, y: i32, w: i32| {
            create_control(
                hwnd,
                "STATIC",
                text,
                0,
                x,
                y,
                w,
                22,
                WINDOW_STYLE(SS_LEFT.0),
                instance,
            )
        };
        let _ = mk_label("Rows (3-5)", 16, 14, 160);
        let rows = create_control(
            hwnd,
            "EDIT",
            "",
            IDC_ROWS,
            200,
            12,
            80,
            22,
            combo(&[WS_BORDER.0, ES_AUTOHSCROLL as u32, ES_NUMBER as u32]),
            instance,
        );
        let _ = mk_label("Columns (3-5)", 16, 44, 160);
        let cols = create_control(
            hwnd,
            "EDIT",
            "",
            IDC_COLS,
            200,
            42,
            80,
            22,
            combo(&[WS_BORDER.0, ES_AUTOHSCROLL as u32, ES_NUMBER as u32]),
            instance,
        );
        let _ = mk_label("Overlay opacity (0.25-0.95)", 16, 74, 180);
        let opacity = create_control(
            hwnd,
            "EDIT",
            "",
            IDC_OPACITY,
            200,
            72,
            80,
            22,
            combo(&[WS_BORDER.0, ES_AUTOHSCROLL as u32]),
            instance,
        );
        let _ = mk_label("Free-mode step (6-90)", 16, 104, 180);
        let step = create_control(
            hwnd,
            "EDIT",
            "",
            IDC_STEP,
            200,
            102,
            80,
            22,
            combo(&[WS_BORDER.0, ES_AUTOHSCROLL as u32, ES_NUMBER as u32]),
            instance,
        );
        let continuous = create_control(
            hwnd,
            "BUTTON",
            "Keep overlay visible after actions",
            IDC_CONTINUOUS,
            16,
            134,
            280,
            22,
            WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            instance,
        );
        let launch = create_control(
            hwnd,
            "BUTTON",
            "Launch at Startup",
            IDC_LAUNCH,
            16,
            162,
            280,
            22,
            WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            instance,
        );

        let _ = mk_label("Overlay shortcut key", 16, 196, 180);
        let hotkey_key = create_control(
            hwnd,
            "EDIT",
            "",
            IDC_HOTKEY_KEY,
            200,
            194,
            60,
            22,
            combo(&[WS_BORDER.0, ES_AUTOHSCROLL as u32]),
            instance,
        );
        let hotkey_ctrl = create_control(
            hwnd,
            "BUTTON",
            "Ctrl",
            IDC_HOTKEY_CTRL,
            16,
            224,
            90,
            22,
            WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            instance,
        );
        let hotkey_alt = create_control(
            hwnd,
            "BUTTON",
            "Alt",
            IDC_HOTKEY_ALT,
            110,
            224,
            90,
            22,
            WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            instance,
        );
        let hotkey_shift = create_control(
            hwnd,
            "BUTTON",
            "Shift",
            IDC_HOTKEY_SHIFT,
            204,
            224,
            90,
            22,
            WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            instance,
        );
        let hotkey_win = create_control(
            hwnd,
            "BUTTON",
            "Win",
            IDC_HOTKEY_WIN,
            300,
            224,
            90,
            22,
            WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            instance,
        );

        let _ = mk_label("Quit grid key", 16, 256, 180);
        let quitkey = create_control(
            hwnd,
            "EDIT",
            "",
            IDC_QUITKEY,
            200,
            254,
            60,
            22,
            combo(&[WS_BORDER.0, ES_AUTOHSCROLL as u32]),
            instance,
        );

        let _ = create_control(
            hwnd,
            "BUTTON",
            "Done",
            IDC_DONE,
            160,
            300,
            100,
            30,
            WINDOW_STYLE(BS_PUSHBUTTON as u32),
            instance,
        );

        set_text(rows, &s.grid_rows.to_string());
        set_text(cols, &s.grid_columns.to_string());
        set_text(opacity, &format!("{:.2}", s.overlay_opacity));
        set_text(step, &s.free_mode_step.to_string());
        set_checked(continuous, s.continuous_mode);
        set_checked(launch, (ctx.launch_get)());
        set_text(hotkey_key, &s.overlay_hotkey.key);
        set_checked(hotkey_ctrl, s.overlay_hotkey.control);
        set_checked(hotkey_alt, s.overlay_hotkey.alt);
        set_checked(hotkey_shift, s.overlay_hotkey.shift);
        set_checked(hotkey_win, s.overlay_hotkey.win);
        set_text(quitkey, &s.quit_grid_key);

        let ctrl = Controls {
            rows,
            cols,
            opacity,
            step,
            continuous,
            launch,
            hotkey_key,
            hotkey_ctrl,
            hotkey_alt,
            hotkey_shift,
            hotkey_win,
            quitkey,
        };
        let state = Box::new(PrefsState { ctx, ctrl });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
        ShowWindow(hwnd, SW_SHOW);
    }
}
