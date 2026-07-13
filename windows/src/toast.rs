use std::ptr::null_mut;
use std::sync::OnceLock;
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, POINT, SIZE, WPARAM, LPARAM, LRESULT};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION, CreateCompatibleDC, CreateDIBSection, CreateFontW,
    DeleteDC, DeleteObject, DrawTextW, GetDC, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE, DT_VCENTER, HBITMAP, HDC, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, KillTimer, RegisterClassW, SetTimer,
    ShowWindow, UpdateLayeredWindow, SW_HIDE, WNDCLASS_STYLES, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WM_TIMER,
};
use crate::common::rgb;

const CLASS_NAME: &str = "MouselessToast";
const TIMER_ID: usize = 1;
const WIDTH: i32 = 260;
const HEIGHT: i32 = 60;

fn class_name() -> windows::core::PCWSTR {
    static NAME: OnceLock<Vec<u16>> = OnceLock::new();
    let v = NAME.get_or_init(|| {
        CLASS_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    });
    unsafe { windows::core::PCWSTR::from_raw(v.as_ptr()) }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

struct ToastWindow {
    hwnd: HWND,
    hdc: HDC,
    bitmap: HBITMAP,
    bits: *mut u32,
}

impl ToastWindow {
    fn create() -> Option<ToastWindow> {
        unsafe {
            let instance = HINSTANCE(
                windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                    .unwrap_or_default()
                    .0,
            );
            let wc = WNDCLASSW {
                style: WNDCLASS_STYLES(0),
                lpfnWndProc: Some(toast_wnd_proc),
                hInstance: instance,
                lpszClassName: class_name(),
                ..Default::default()
            };
            let _ = RegisterClassW(&wc);

            let hwnd = match CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
                class_name(),
                windows::core::PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                WIDTH,
                HEIGHT,
                None,
                None,
                instance,
                None,
            ) {
                Ok(h) => h,
                Err(_) => return None,
            };
            let screen_dc = GetDC(HWND::default());
            let hdc = CreateCompatibleDC(screen_dc);
            let _ = windows::Win32::Graphics::Gdi::ReleaseDC(HWND::default(), screen_dc);
            if hdc.is_invalid() {
                let _ = DestroyWindow(hwnd);
                return None;
            }
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: WIDTH,
                biHeight: -HEIGHT,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                ..Default::default()
            };
            let mut bits: *mut std::ffi::c_void = null_mut();
            let bitmap = match CreateDIBSection(
                hdc,
                &bmi,
                DIB_RGB_COLORS,
                &mut bits,
                HANDLE(null_mut()),
                0,
            ) {
                Ok(b) => b,
                Err(_) => {
                    let _ = DeleteDC(hdc);
                    let _ = DestroyWindow(hwnd);
                    return None;
                }
            };
            let _ = SelectObject(hdc, bitmap);
            Some(ToastWindow {
                hwnd,
                hdc,
                bitmap,
                bits: bits as *mut u32,
            })
        }
    }

    fn show(&self, text: &str) {
        unsafe {
            let buf = std::slice::from_raw_parts_mut(self.bits, (WIDTH * HEIGHT) as usize);
            for p in buf.iter_mut() {
                *p = 0;
            }
            // Dark rounded-ish background.
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let idx = (y as usize) * (WIDTH as usize) + x as usize;
                    buf[idx] = ((210u32) << 24) | ((20 << 16) | (20 << 8) | 24);
                }
            }
            let mut wide = to_wide(text);
            let hfont = CreateFontW(
                -16, 0, 0, 0, 600, 0,
                0, 0, 1, 0, 0, 0,
                0, windows::core::PCWSTR::null(),
            );
            let old = SelectObject(self.hdc, hfont);
            SetBkMode(self.hdc, TRANSPARENT);
            SetTextColor(self.hdc, rgb(255, 255, 255));
            let mut rect = windows::Win32::Foundation::RECT {
                left: 6,
                top: 0,
                right: WIDTH - 6,
                bottom: HEIGHT,
            };
            let _ = DrawTextW(
                self.hdc,
                &mut wide,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            let _ = SelectObject(self.hdc, old);
            let _ = DeleteObject(hfont);

            // Promote GDI text to opaque.
            for p in buf.iter_mut() {
                let a = (*p >> 24) & 0xff;
                let rgb = *p & 0x00ff_ffff;
                if a == 0 && rgb != 0 {
                    *p = rgb | (255u32 << 24);
                }
            }

            let point_dst = POINT {
                x: 0,
                y: 0,
            };
            let size = SIZE {
                cx: WIDTH,
                cy: HEIGHT,
            };
            let pt_src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let _ = UpdateLayeredWindow(
                self.hwnd,
                HDC::default(),
                Some(&point_dst),
                Some(&size),
                self.hdc,
                Some(&pt_src),
                rgb(0, 0, 0),
                Some(&blend),
                windows::Win32::UI::WindowsAndMessaging::ULW_ALPHA,
            );
            // Position near the top-center of the primary monitor.
            let mx = windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN,
            );
            let px = (mx - WIDTH) / 2;
            let py = 80;
            let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowPos(
                self.hwnd,
                windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST,
                px,
                py,
                WIDTH,
                HEIGHT,
                windows::Win32::UI::WindowsAndMessaging::SWP_SHOWWINDOW
                    | windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
            );
            let _ = SetTimer(self.hwnd, TIMER_ID, 1200, None);
        }
    }
}

impl Drop for ToastWindow {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.is_invalid() {
                let _ = DestroyWindow(self.hwnd);
            }
            if !self.bitmap.is_invalid() {
                let _ = DeleteObject(self.bitmap);
            }
            if !self.hdc.is_invalid() {
                let _ = DeleteDC(self.hdc);
            }
        }
    }
}

unsafe impl Send for ToastWindow {}
unsafe impl Sync for ToastWindow {}

extern "system" fn toast_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_TIMER && wparam.0 as usize == TIMER_ID {
        unsafe {
            let _ = KillTimer(hwnd, TIMER_ID);
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

static TOAST: OnceLock<ToastWindow> = OnceLock::new();

pub fn show_toast(text: &str) {
    let toast = TOAST.get_or_init(|| ToastWindow::create().expect("toast window"));
    toast.show(text);
}
