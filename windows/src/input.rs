use std::sync::OnceLock;
use windows::Win32::Foundation::{HINSTANCE, LRESULT, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{KBDLLHOOKSTRUCT, WH_KEYBOARD_LL};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HHOOK, SetWindowsHookExW, UnhookWindowsHookEx, WM_KEYDOWN,
    WM_SYSKEYDOWN,
};

pub const VK_MENU: u32 = 0x12;
pub const VK_CONTROL: u32 = 0x11;
pub const VK_SHIFT: u32 = 0x10;
pub const VK_LWIN: u32 = 0x5B;
pub const VK_RWIN: u32 = 0x5C;
// Low-level keyboard hooks report side-specific modifier codes.
pub const VK_LSHIFT: u32 = 0xA0;
pub const VK_RSHIFT: u32 = 0xA1;
pub const VK_LCONTROL: u32 = 0xA2;
pub const VK_RCONTROL: u32 = 0xA3;
pub const VK_LMENU: u32 = 0xA4;
pub const VK_RMENU: u32 = 0xA5;

type KeyCallback = Box<dyn Fn(u32, bool) -> i32 + Send + Sync>;

static CALLBACK: OnceLock<KeyCallback> = OnceLock::new();
static mut HOOK: Option<HHOOK> = None;

pub fn install_keyboard_hook(cb: KeyCallback) -> bool {
    let _ = CALLBACK.set(cb);
    unsafe {
        let inst = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), inst, 0);
        match hook {
            Ok(h) => {
                HOOK = Some(h);
                true
            }
            Err(_) => false,
        }
    }
}

pub fn uninstall_keyboard_hook() {
    unsafe {
        if let Some(h) = HOOK.take() {
            let _ = UnhookWindowsHookEx(h);
        }
    }
}

extern "system" fn low_level_keyboard_proc(
    ncode: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if ncode >= 0 {
        let kb = unsafe { *(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = kb.vkCode;
        let msg = wparam.0 as u32;
        let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        if let Some(cb) = CALLBACK.get() {
            let consume = cb(vk, is_down);
            if consume != 0 {
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
}

/// Returns the live state of the modifier keys (ctrl, alt, shift, win),
/// accounting for both left and right hand-side keys. Querying the actual
/// key state avoids stale or missed key-up events that can otherwise break
/// hotkey detection.
pub fn current_modifiers() -> (bool, bool, bool, bool) {
    let ctrl =
        key_down(VK_LCONTROL as i32) || key_down(VK_RCONTROL as i32) || key_down(VK_CONTROL as i32);
    let alt = key_down(VK_LMENU as i32) || key_down(VK_RMENU as i32) || key_down(VK_MENU as i32);
    let shift =
        key_down(VK_LSHIFT as i32) || key_down(VK_RSHIFT as i32) || key_down(VK_SHIFT as i32);
    let win = key_down(VK_LWIN as i32) || key_down(VK_RWIN as i32);
    (ctrl, alt, shift, win)
}

fn key_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16) & 0x8000 != 0 }
}

/// Maps a Windows virtual-key code to the logical label used by the core logic.
pub fn label_for_vk(vk: u32) -> Option<String> {
    let label = match vk {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x10 => "Shift",
        0x11 => "Control",
        0x12 => "Alt",
        0x13 => "Pause",
        0x14 => "CapsLock",
        0x1B => "Escape",
        0x20 => "Space",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "ArrowLeft",
        0x26 => "ArrowUp",
        0x27 => "ArrowRight",
        0x28 => "ArrowDown",
        0x2D => "Insert",
        0x2E => "Delete",
        0x30..=0x39 => return Some(((b'0' as u32 + (vk - 0x30)) as u8 as char).to_string()),
        0x41..=0x5A => return Some(((b'A' as u32 + (vk - 0x41)) as u8 as char).to_string()),
        0x5B => "LWin",
        0x5C => "RWin",
        0x60..=0x69 => return Some(((b'0' as u32 + (vk - 0x60)) as u8 as char).to_string()),
        0x70..=0x7B => return Some(format!("F{}", vk - 0x6F)),
        0xBA => ";",
        0xBB => "=",
        0xBC => ",",
        0xBD => "-",
        0xBE => ".",
        0xBF => "/",
        0xC0 => "`",
        0xDB => "[",
        0xDC => "\\",
        0xDD => "]",
        0xDE => "'",
        0xE2 => "\\",
        _ => return None,
    };
    Some(label.to_string())
}
