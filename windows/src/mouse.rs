use crate::common::MouseButton;
use crate::core::MouseBackend;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

const WHEEL_SCALE: i32 = 6;

pub struct SendInputMouse {
    dragging: AtomicBool,
}

impl SendInputMouse {
    pub fn new() -> Self {
        SendInputMouse {
            dragging: AtomicBool::new(false),
        }
    }

    fn virtual_screen() -> (i32, i32, i32, i32) {
        unsafe {
            (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        }
    }

    fn to_absolute(x: f64, y: f64) -> (i32, i32) {
        let (vx, vy, vw, vh) = Self::virtual_screen();
        let vw = (vw - 1).max(1);
        let vh = (vh - 1).max(1);
        let nx = ((x - vx as f64) * 65535.0 / vw as f64).clamp(0.0, 65535.0) as i32;
        let ny = ((y - vy as f64) * 65535.0 / vh as f64).clamp(0.0, 65535.0) as i32;
        (nx, ny)
    }

    fn send(inputs: &[INPUT]) {
        unsafe {
            SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn move_event(x: f64, y: f64) -> INPUT {
        let (nx, ny) = Self::to_absolute(x, y);
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: nx,
                    dy: ny,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn button_event(down: bool, button: MouseButton, x: f64, y: f64) -> INPUT {
        let (nx, ny) = Self::to_absolute(x, y);
        let flags = match (button, down) {
            (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
            (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
            (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
            (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
        };
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: nx,
                    dy: ny,
                    mouseData: 0,
                    dwFlags: flags | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }
}

impl MouseBackend for SendInputMouse {
    fn move_to(&self, x: f64, y: f64) {
        Self::send(&[Self::move_event(x, y)]);
    }

    fn click(&self, x: f64, y: f64, button: MouseButton, count: u32) {
        self.move_to(x, y);
        sleep(Duration::from_millis(30));
        let down = match button {
            MouseButton::Left => MOUSEEVENTF_LEFTDOWN,
            MouseButton::Right => MOUSEEVENTF_RIGHTDOWN,
        };
        let up = match button {
            MouseButton::Left => MOUSEEVENTF_LEFTUP,
            MouseButton::Right => MOUSEEVENTF_RIGHTUP,
        };
        let _ = (down, up);
        for _ in 0..count.max(1) {
            Self::send(&[Self::button_event(true, button, x, y)]);
            sleep(Duration::from_millis(50));
            Self::send(&[Self::button_event(false, button, x, y)]);
            sleep(Duration::from_millis(10));
        }
    }

    fn toggle_drag(&self, x: f64, y: f64) {
        if self.dragging.load(Ordering::SeqCst) {
            self.move_drag(x, y);
            Self::send(&[Self::button_event(false, MouseButton::Left, x, y)]);
            self.dragging.store(false, Ordering::SeqCst);
        } else {
            self.move_to(x, y);
            Self::send(&[Self::button_event(true, MouseButton::Left, x, y)]);
            self.dragging.store(true, Ordering::SeqCst);
        }
    }

    fn move_drag(&self, x: f64, y: f64) {
        if self.dragging.load(Ordering::SeqCst) {
            Self::send(&[Self::move_event(x, y)]);
        }
    }

    fn scroll(&self, vertical: i32, horizontal: i32) {
        if vertical != 0 {
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: vertical as u32,
                        dwFlags: MOUSEEVENTF_WHEEL,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            Self::send(&[input]);
        }
        if horizontal != 0 {
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: horizontal as u32,
                        dwFlags: MOUSEEVENTF_HWHEEL,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            Self::send(&[input]);
        }
    }

    fn scroll_at(&self, x: f64, y: f64, vertical: i32, horizontal: i32) {
        self.move_to(x, y);
        self.scroll(vertical, horizontal);
    }

    fn is_dragging(&self) -> bool {
        self.dragging.load(Ordering::SeqCst)
    }
}

#[allow(dead_code)]
fn _thread_id() -> u32 {
    unsafe { GetCurrentThreadId() }
}
