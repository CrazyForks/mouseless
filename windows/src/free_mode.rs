use crate::common::MouseButton;
use crate::core::{Environment, MouseBackend};
use crate::settings::Settings;
use std::sync::Arc;

pub struct FreeModeController {
    active: bool,
    mouse: Arc<dyn MouseBackend>,
    env: Arc<dyn Environment>,
    notify: Arc<dyn Fn(&str) + Send + Sync>,
}

impl FreeModeController {
    pub fn new(
        mouse: Arc<dyn MouseBackend>,
        env: Arc<dyn Environment>,
        notify: Arc<dyn Fn(&str) + Send + Sync>,
    ) -> Self {
        FreeModeController {
            active: false,
            mouse,
            env,
            notify,
        }
    }

    pub fn toggle(&mut self) {
        if self.active {
            self.exit();
        } else {
            self.enter();
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn enter(&mut self) {
        self.active = true;
        (self.notify)("Free mode");
    }

    pub fn exit(&mut self) {
        self.active = false;
        (self.notify)("Free mode off");
    }

    // The click sleeps must not block the low-level keyboard hook thread.
    fn click_async(&self, x: f64, y: f64, button: MouseButton) {
        let mouse = self.mouse.clone();
        std::thread::spawn(move || mouse.click(x, y, button, 1));
    }

    pub fn handle(&mut self, label: &str, settings: &Settings) -> bool {
        if !self.active {
            return false;
        }
        let step = settings.free_mode_step;
        let (mut x, mut y) = self.env.cursor_pos();
        match label {
            "Escape" => self.exit(),
            "H" | "ArrowLeft" => {
                x -= step;
                self.mouse.move_to(x, y);
            }
            "L" | "ArrowRight" => {
                x += step;
                self.mouse.move_to(x, y);
            }
            "K" | "ArrowUp" => {
                y -= step;
                self.mouse.move_to(x, y);
            }
            "J" | "ArrowDown" => {
                y += step;
                self.mouse.move_to(x, y);
            }
            "Space" => self.click_async(x, y, MouseButton::Left),
            "R" => self.click_async(x, y, MouseButton::Right),
            "U" => self.mouse.scroll(settings.scroll_step * 6, 0),
            "D" => self.mouse.scroll(-(settings.scroll_step * 6), 0),
            "Y" => self.mouse.scroll(0, -(settings.scroll_step * 6)),
            "O" => self.mouse.scroll(0, settings.scroll_step * 6),
            _ => {}
        }
        true
    }
}
