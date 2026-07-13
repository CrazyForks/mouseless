use crate::common::Rect;
use crate::core::Environment;
use std::ptr;
use windows::Win32::Foundation::{BOOL, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

pub struct SystemEnvironment;

impl SystemEnvironment {
    pub fn new() -> Self {
        SystemEnvironment
    }
}

impl Environment for SystemEnvironment {
    fn monitors(&self) -> Vec<Rect> {
        let mut rects: Vec<Rect> = Vec::new();
        unsafe {
            let ptr = &mut rects as *mut Vec<Rect>;
            let _ = EnumDisplayMonitors(
                HDC(ptr::null_mut()),
                None,
                Some(monitor_enum_proc),
                LPARAM(ptr as isize),
            );
        }
        rects
    }

    fn cursor_pos(&self) -> (f64, f64) {
        let mut p = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut p);
        }
        (p.x as f64, p.y as f64)
    }
}

extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _lprect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let rects = unsafe { &mut *(lparam.0 as *mut Vec<Rect>) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(hmonitor, &mut info) }.as_bool() {
        let r = info.rcMonitor;
        rects.push(Rect::new(
            r.left as f64,
            r.top as f64,
            r.right as f64,
            r.bottom as f64,
        ));
    }
    BOOL(1)
}
