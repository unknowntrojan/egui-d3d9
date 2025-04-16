#![allow(dead_code)]
use clipboard::{windows_clipboard::WindowsClipboardContext, ClipboardProvider};
use egui::{
    Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, Theme, Vec2,
};
use windows::{
    Wdk::System::SystemInformation::NtQuerySystemTime,
    Win32::{
        Foundation::{HWND, RECT},
        System::SystemServices::{MK_CONTROL, MK_SHIFT},
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END,
                VK_ESCAPE, VK_HOME, VK_INSERT, VK_LEFT, VK_LSHIFT, VK_NEXT, VK_PRIOR, VK_RETURN,
                VK_RIGHT, VK_SPACE, VK_TAB, VK_UP,
            },
            WindowsAndMessaging::{
                GetClientRect, KF_REPEAT, WHEEL_DELTA, WM_CHAR, WM_KEYDOWN, WM_KEYUP,
                WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN,
                WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDBLCLK,
                WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDBLCLK,
                WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
            },
        },
    },
};

pub struct InputManager {
    hwnd: HWND,
    events: Vec<Event>,
    modifiers: Option<Modifiers>,
}

/// High-level overview of recognized `WndProc` messages.
#[repr(u8)]
pub enum InputResult {
    Unknown,
    MouseMove,
    MouseLeft,
    MouseRight,
    MouseMiddle,
    Character,
    Scroll,
    Zoom,
    Key,
}

impl InputResult {
    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.is_unknown()
    }

    #[inline]
    pub fn is_unknown(&self) -> bool {
        matches!(*self, InputResult::Unknown)
    }
}

impl InputManager {
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            events: vec![],
            modifiers: None,
        }
    }

    pub fn process(&mut self, umsg: u32, wparam: usize, lparam: isize) -> InputResult {
        match umsg {
            WM_MOUSEMOVE => {
                self.alter_modifiers(get_mouse_modifiers(wparam));

                self.events.push(Event::PointerMoved(get_pos(lparam)));
                InputResult::MouseMove
            }
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers,
                });
                InputResult::MouseLeft
            }
            WM_LBUTTONUP => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers,
                });
                InputResult::MouseLeft
            }
            WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: PointerButton::Secondary,
                    pressed: true,
                    modifiers,
                });
                InputResult::MouseRight
            }
            WM_RBUTTONUP => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: PointerButton::Secondary,
                    pressed: false,
                    modifiers,
                });
                InputResult::MouseRight
            }
            WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: PointerButton::Middle,
                    pressed: true,
                    modifiers,
                });
                InputResult::MouseMiddle
            }
            WM_MBUTTONUP => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: PointerButton::Middle,
                    pressed: false,
                    modifiers,
                });
                InputResult::MouseMiddle
            }
            WM_XBUTTONDOWN | WM_XBUTTONDBLCLK => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: if (wparam as u32) >> 16 & (XBUTTON1 as u32) != 0 {
                        PointerButton::Extra1
                    } else if (wparam as u32) >> 16 & (XBUTTON2 as u32) != 0 {
                        PointerButton::Extra2
                    } else {
                        unreachable!()
                    },
                    pressed: true,
                    modifiers,
                });
                InputResult::MouseMiddle
            }
            WM_XBUTTONUP => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                self.events.push(Event::PointerButton {
                    pos: get_pos(lparam),
                    button: if (wparam as u32) >> 16 & (XBUTTON1 as u32) != 0 {
                        PointerButton::Extra1
                    } else if (wparam as u32) >> 16 & (XBUTTON2 as u32) != 0 {
                        PointerButton::Extra2
                    } else {
                        unreachable!()
                    },
                    pressed: false,
                    modifiers,
                });
                InputResult::MouseMiddle
            }
            WM_CHAR => {
                if let Some(ch) = char::from_u32(wparam as _) {
                    if !ch.is_control() {
                        self.events.push(Event::Text(ch.into()));
                    }
                }
                InputResult::Character
            }
            WM_MOUSEWHEEL => {
                self.alter_modifiers(get_mouse_modifiers(wparam));

                let delta = (wparam >> 16) as i16 as f32 * 10. / WHEEL_DELTA as f32;

                if wparam & MK_CONTROL.0 as usize != 0 {
                    self.events
                        .push(Event::Zoom(if delta > 0. { 1.5 } else { 0.5 }));
                    InputResult::Zoom
                } else {
                    self.events.push(Event::MouseWheel {
                        unit: MouseWheelUnit::Point,
                        delta: Vec2::new(0., delta),
                        modifiers: get_mouse_modifiers(wparam),
                    });
                    InputResult::Scroll
                }
            }
            WM_MOUSEHWHEEL => {
                self.alter_modifiers(get_mouse_modifiers(wparam));

                let delta = (wparam >> 16) as i16 as f32 * 10. / WHEEL_DELTA as f32;

                if wparam & MK_CONTROL.0 as usize != 0 {
                    self.events
                        .push(Event::Zoom(if delta > 0. { 1.5 } else { 0.5 }));
                    InputResult::Zoom
                } else {
                    self.events.push(Event::MouseWheel {
                        unit: MouseWheelUnit::Point,
                        delta: Vec2::new(delta, 0.),
                        modifiers: get_mouse_modifiers(wparam),
                    });
                    InputResult::Scroll
                }
            }
            msg @ (WM_KEYDOWN | WM_SYSKEYDOWN) => {
                let modifiers = get_key_modifiers(msg);
                self.modifiers = Some(modifiers);

                if let Some(key) = get_key(wparam) {
                    if key == Key::V && modifiers.ctrl {
                        if let Some(clipboard) = get_clipboard_text() {
                            self.events.push(Event::Text(clipboard));
                        }
                    }

                    if key == Key::C && modifiers.ctrl {
                        self.events.push(Event::Copy);
                    }

                    if key == Key::X && modifiers.ctrl {
                        self.events.push(Event::Cut);
                    }

                    self.events.push(Event::Key {
                        pressed: true,
                        physical_key: None,
                        modifiers,
                        key,
                        repeat: lparam & (KF_REPEAT as isize) > 0,
                    });
                }
                InputResult::Key
            }
            msg @ (WM_KEYUP | WM_SYSKEYUP) => {
                let modifiers = get_key_modifiers(msg);
                self.modifiers = Some(modifiers);

                if let Some(key) = get_key(wparam) {
                    self.events.push(Event::Key {
                        pressed: false,
                        physical_key: None,
                        modifiers,
                        key,
                        repeat: false,
                    });
                }
                InputResult::Key
            }
            _ => InputResult::Unknown,
        }
    }

    fn alter_modifiers(&mut self, new: Modifiers) {
        if let Some(old) = self.modifiers.as_mut() {
            *old = new;
        }
    }

    pub fn collect_input(&mut self) -> RawInput {
        RawInput {
            modifiers: self.modifiers.unwrap_or_default(),
            events: self.events.drain(..).collect::<Vec<Event>>(),
            screen_rect: Some(self.get_screen_rect()),
            time: Some(Self::get_system_time()),
            system_theme: get_system_theme(),
            max_texture_side: None,
            predicted_dt: 1. / 60.,
            hovered_files: vec![],
            dropped_files: vec![],
            focused: true,
            ..Default::default()
        }
    }

    /// Returns time in seconds.
    pub fn get_system_time() -> f64 {
        let mut time = 0;
        unsafe {
            expect!(
                NtQuerySystemTime(&mut time).ok(),
                "Failed to get system time"
            );
        }

        // dumb ass, read the docs. egui clearly says `in seconds`.
        // Shouldn't have wasted 3 days on this.
        // `NtQuerySystemTime` returns how many 100 nanosecond intervals
        // past since 1st Jan, 1601.
        (time as f64) / 10_000_000.
    }

    #[inline]
    pub fn get_screen_size(&self) -> Pos2 {
        let mut rect = RECT::default();
        unsafe {
            expect!(
                GetClientRect(self.hwnd, &mut rect),
                "Failed to GetClientRect()"
            );
        }

        Pos2::new(
            (rect.right - rect.left) as f32,
            (rect.bottom - rect.top) as f32,
        )
    }

    #[inline]
    pub fn get_screen_rect(&self) -> Rect {
        Rect {
            min: Pos2::ZERO,
            max: self.get_screen_size(),
        }
    }
}

fn get_pos(lparam: isize) -> Pos2 {
    let x = (lparam & 0xFFFF) as i16 as f32;
    let y = (lparam >> 16 & 0xFFFF) as i16 as f32;

    Pos2::new(x, y)
}

fn get_mouse_modifiers(wparam: usize) -> Modifiers {
    Modifiers {
        alt: false,
        ctrl: (wparam & MK_CONTROL.0 as usize) != 0,
        shift: (wparam & MK_SHIFT.0 as usize) != 0,
        mac_cmd: false,
        command: (wparam & MK_CONTROL.0 as usize) != 0,
    }
}

fn get_key_modifiers(msg: u32) -> Modifiers {
    let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL.0 as _) != 0 };
    let shift = unsafe { GetAsyncKeyState(VK_LSHIFT.0 as _) != 0 };

    Modifiers {
        alt: msg == WM_SYSKEYDOWN,
        mac_cmd: false,
        command: ctrl,
        shift,
        ctrl,
    }
}

fn get_key(wparam: usize) -> Option<Key> {
    match wparam {
        0x08 => Some(Key::Backspace),
        0x09 => Some(Key::Tab),
        0x0D => Some(Key::Enter),
        0x1B => Some(Key::Escape),
        0x20 => Some(Key::Space),
        0x21 => Some(Key::PageUp),
        0x22 => Some(Key::PageDown),
        0x23 => Some(Key::End),
        0x24 => Some(Key::Home),
        0x25 => Some(Key::ArrowLeft),
        0x26 => Some(Key::ArrowUp),
        0x27 => Some(Key::ArrowRight),
        0x28 => Some(Key::ArrowDown),
        0x2D => Some(Key::Insert),
        0x2E => Some(Key::Delete),
        0x30 => Some(Key::Num0),
        0x31 => Some(Key::Num1),
        0x32 => Some(Key::Num2),
        0x33 => Some(Key::Num3),
        0x34 => Some(Key::Num4),
        0x35 => Some(Key::Num5),
        0x36 => Some(Key::Num6),
        0x37 => Some(Key::Num7),
        0x38 => Some(Key::Num8),
        0x39 => Some(Key::Num9),
        0x41 => Some(Key::A),
        0x42 => Some(Key::B),
        0x43 => Some(Key::C),
        0x44 => Some(Key::D),
        0x45 => Some(Key::E),
        0x46 => Some(Key::F),
        0x47 => Some(Key::G),
        0x48 => Some(Key::H),
        0x49 => Some(Key::I),
        0x4A => Some(Key::J),
        0x4B => Some(Key::K),
        0x4C => Some(Key::L),
        0x4D => Some(Key::M),
        0x4E => Some(Key::N),
        0x4F => Some(Key::O),
        0x50 => Some(Key::P),
        0x51 => Some(Key::Q),
        0x52 => Some(Key::R),
        0x53 => Some(Key::S),
        0x54 => Some(Key::T),
        0x55 => Some(Key::U),
        0x56 => Some(Key::V),
        0x57 => Some(Key::W),
        0x58 => Some(Key::X),
        0x59 => Some(Key::Y),
        0x5A => Some(Key::Z),
        0x70 => Some(Key::F1),
        0x71 => Some(Key::F2),
        0x72 => Some(Key::F3),
        0x73 => Some(Key::F4),
        0x74 => Some(Key::F5),
        0x75 => Some(Key::F6),
        0x76 => Some(Key::F7),
        0x77 => Some(Key::F8),
        0x78 => Some(Key::F9),
        0x79 => Some(Key::F10),
        0x7A => Some(Key::F11),
        0x7B => Some(Key::F12),
        0x7C => Some(Key::F13),
        0x7D => Some(Key::F14),
        0x7E => Some(Key::F15),
        0x7F => Some(Key::F16),
        0x80 => Some(Key::F17),
        0x81 => Some(Key::F18),
        0x82 => Some(Key::F19),
        0x83 => Some(Key::F20),
        0x84 => Some(Key::F21),
        0x85 => Some(Key::F22),
        0x86 => Some(Key::F23),
        0x87 => Some(Key::F24),
        0xBA => Some(Key::Semicolon),
        0xBB => Some(Key::Equals),
        0xBC => Some(Key::Comma),
        0xBD => Some(Key::Minus),
        0xBE => Some(Key::Period),
        0xBF => Some(Key::Slash),
        0xC0 => Some(Key::Backtick),
        0xDB => Some(Key::OpenBracket),
        0xDC => Some(Key::Backslash),
        0xDD => Some(Key::CloseBracket),
        0xDE => Some(Key::Quote),
        _ => None,
    }
}

fn get_system_theme() -> Option<Theme> {
    let key = windows_registry::CURRENT_USER
        .open(
            "HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
        )
        .ok()?;

    Some(if key.get_u32("AppsUseLightTheme").ok()? == 1 {
        Theme::Light
    } else {
        Theme::Dark
    })
}

fn get_clipboard_text() -> Option<String> {
    WindowsClipboardContext.get_contents().ok()
}
