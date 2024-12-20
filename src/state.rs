use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
};

use crate::recorder::RecordEntry;

// struct InputState {
//     time_ms: f64,
//     mouse: MouseState,
//     keyboard: KeyboardState,
//     gamepad: HashMap<u32, ControllerState>,
// }
// struct MouseState {
//     pos: [f64; 2],
//     buttons: [bool; 3],
//     wheel: [f64; 2],
// }
// struct KeyboardState {
//     keys: HashSet<Key>,
// }
// struct ControllerState {
//     buttons: [bool; 16],
//     triggers: [f64; 2],
//     sticks: [[f64; 2]; 2],
// }

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct GlobalState {
    pub time_ms: f64,
    pub pressed_keys: Vec<AnyKey>,
    pub offsets: AllOffsets,

    #[serde(skip)]
    rec_pressed: Vec<AnyKey>,
    #[serde(skip)]
    rec_released: Vec<AnyKey>,
    #[serde(skip)]
    rec_moves: Vec<AnyOffset>,
}

impl From<rdev::Key> for AnyKey {
    fn from(key: rdev::Key) -> Self {
        AnyKey::Keyboard(Key(key))
    }
}
impl From<rdev::Button> for AnyKey {
    fn from(button: rdev::Button) -> Self {
        match button {
            rdev::Button::Left => AnyKey::MouseButton(0),
            rdev::Button::Right => AnyKey::MouseButton(1),
            rdev::Button::Middle => AnyKey::MouseButton(2),
            rdev::Button::Unknown(i) => AnyKey::MouseButton(i as u32),
        }
    }
}
// From controller
impl From<(u32, usize)> for AnyKey {
    fn from(button: (u32, usize)) -> Self {
        AnyKey::Controller(button.0, button.1)
    }
}

impl GlobalState {
    pub fn key_down(&mut self, key: AnyKey) {
        self.rec_pressed.push(key.clone());
        if !self.pressed_keys.contains(&key) {
            self.pressed_keys.push(key);
        }
    }
    pub fn key_up(&mut self, key: AnyKey) {
        self.pressed_keys.retain(|k| k != &key);
        self.rec_released.push(key);
    }
    pub fn moves(&mut self, offset: AnyOffset) {
        match offset {
            AnyOffset::Mouse(x, y) => self.offsets.mouse = (x, y),
            AnyOffset::Wheel(x, y) => self.offsets.wheel = (x, y),
            AnyOffset::Trigger(i, x, y) => self.offsets.trigger[i as usize] = (x, y),
            AnyOffset::LeftStick(i, x, y) => self.offsets.left_stick[i as usize] = (x, y),
            AnyOffset::RightStick(i, x, y) => self.offsets.right_stick[i as usize] = (x, y),
        }
        self.rec_moves.push(offset);
    }

    pub fn next_ms(&mut self, ms: f64) -> RecordEntry {
        let pressed = std::mem::replace(&mut self.rec_pressed, Vec::new());
        let released = std::mem::replace(&mut self.rec_released, Vec::new());
        let moves = std::mem::replace(&mut self.rec_moves, Vec::new());
        let res = RecordEntry {
            ms,
            pressed,
            released,
            moves,
        };
        self.time_ms = ms;
        res
    }

    pub fn get_pattern(&self) -> ShortCut {
        let mut res = ShortCut::NONE;
        for key in &self.pressed_keys {
            match key {
                AnyKey::Keyboard(Key(k)) => match k {
                    rdev::Key::ControlLeft | rdev::Key::ControlRight => res.ctrl = Some(true),
                    rdev::Key::Alt | rdev::Key::AltGr => res.alt = Some(true),
                    rdev::Key::ShiftLeft | rdev::Key::ShiftRight => res.shift = Some(true),
                    rdev::Key::Tab => res.tab = Some(true),
                    rdev::Key::MetaLeft | rdev::Key::MetaRight => res.windows = Some(true),
                    _ => (),
                },
                AnyKey::MouseButton(i) => match i {
                    0 => res.mouse_l_button = Some(true),
                    1 => res.mouse_r_button = Some(true),
                    2 => res.mouse_m_button = Some(true),
                    _ => (),
                },
                _ => (),
            }
        }
        res
    }

    pub fn match_shortcut(&self, pat: &ShortCut, shortcut: &ShortCut) -> bool {
        // compare mods
        fn cmp(t: &Option<bool>, s: &Option<bool>) -> bool {
            s.is_none() || t.is_some() == s.unwrap()
        }
        let modifiers = cmp(&pat.alt, &shortcut.alt)
            && cmp(&pat.shift, &shortcut.shift)
            && cmp(&pat.tab, &shortcut.tab)
            && cmp(&pat.windows, &shortcut.windows)
            && cmp(&pat.mouse_l_button, &shortcut.mouse_l_button)
            && cmp(&pat.mouse_r_button, &shortcut.mouse_r_button)
            && cmp(&pat.mouse_m_button, &shortcut.mouse_m_button);
        if !modifiers {
            return false;
        }
        // compare triggers
        if let Some(i) = shortcut.trigger_l {
            if self.offsets.trigger[i as usize].0 == 0.0 {
                return false;
            }
        }
        if let Some(i) = shortcut.trigger_r {
            if self.offsets.trigger[i as usize].1 == 0.0 {
                return false;
            }
        }
        // compare key
        if let Some(k) = &shortcut.key {
            if !self.pressed_keys.contains(&AnyKey::Keyboard(k.clone())) {
                return false;
            }
        }
        // compare controller button
        if let Some((id, index)) = shortcut.controller_btn {
            if !self.pressed_keys.contains(&AnyKey::Controller(id, index)) {
                return false;
            }
        }
        true
    }

    pub fn match_shortcuts(&self, pat: &ShortCut, shortcuts: &ShortCuts) -> bool {
        match shortcuts {
            ShortCuts::Contains(vec) => {
                for shortcut in vec {
                    if self.match_shortcut(pat, shortcut) {
                        return true;
                    }
                }
                false
            }
            ShortCuts::Exclude(vec) => {
                for shortcut in vec {
                    if self.match_shortcut(pat, shortcut) {
                        return false;
                    }
                }
                true
            }
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
// struct Key(u32);
pub struct Key(rdev::Key);

#[derive(Serialize, Deserialize, PartialEq)]
pub struct ShortCut {
    /// The key that triggers the shortcut.
    pub key: Option<Key>,
    /// The controller button that triggers the shortcut. (id, button)
    pub controller_btn: Option<(u32, usize)>,
    // The following modifiers are optional because they are not always needed.
    pub ctrl: Option<bool>,
    pub alt: Option<bool>,
    pub shift: Option<bool>,
    pub tab: Option<bool>,
    pub windows: Option<bool>,
    pub mouse_l_button: Option<bool>,
    pub mouse_r_button: Option<bool>,
    pub mouse_m_button: Option<bool>,
    // trigger on the stick of the id'th controller
    pub trigger_l: Option<u32>,
    pub trigger_r: Option<u32>,
}

/// A list of shortcuts that can be used to trigger an action.
/// This struct gives a list used either to include or exclude shortcuts.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum ShortCuts {
    Contains(Vec<ShortCut>),
    Exclude(Vec<ShortCut>),
}

impl ShortCut {
    pub const SHIFT_ENTER: Self = Self {
        key: Some(Key(rdev::Key::Return)),
        controller_btn: None,
        ctrl: Some(false),
        alt: Some(false),
        shift: Some(true),
        tab: Some(false),
        windows: Some(false),
        mouse_l_button: None,
        mouse_r_button: None,
        mouse_m_button: None,
        trigger_l: None,
        trigger_r: None,
    };
    pub const ESCAPE: Self = Self {
        key: Some(Key(rdev::Key::Escape)),
        controller_btn: None,
        ctrl: Some(false),
        alt: Some(false),
        shift: Some(false),
        tab: Some(false),
        windows: Some(false),
        mouse_l_button: None,
        mouse_r_button: None,
        mouse_m_button: None,
        trigger_l: None,
        trigger_r: None,
    };
    pub const SHIFT_ESCAPE: Self = Self {
        key: Some(Key(rdev::Key::Escape)),
        controller_btn: None,
        ctrl: Some(false),
        alt: Some(false),
        shift: Some(true),
        tab: Some(false),
        windows: Some(false),
        mouse_l_button: None,
        mouse_r_button: None,
        mouse_m_button: None,
        trigger_l: None,
        trigger_r: None,
    };
    pub const CTRL_ENTER: Self = Self {
        key: Some(Key(rdev::Key::Return)),
        controller_btn: None,
        ctrl: Some(true),
        alt: Some(false),
        shift: Some(false),
        tab: Some(false),
        windows: Some(false),
        mouse_l_button: None,
        mouse_r_button: None,
        mouse_m_button: None,
        trigger_l: None,
        trigger_r: None,
    };
    pub const CTRL_RIGHT_S: Self = Self {
        key: Some(Key(rdev::Key::KeyS)),
        controller_btn: None,
        ctrl: Some(true),
        alt: Some(false),
        shift: Some(true),
        tab: Some(false),
        windows: Some(false),
        mouse_l_button: None,
        mouse_r_button: Some(true),
        mouse_m_button: None,
        trigger_l: None,
        trigger_r: None,
    };

    pub const NONE: Self = Self {
        key: None,
        controller_btn: None,
        ctrl: None,
        alt: None,
        shift: None,
        tab: None,
        windows: None,
        mouse_l_button: None,
        mouse_r_button: None,
        mouse_m_button: None,
        trigger_l: None,
        trigger_r: None,
    };
}

impl Debug for ShortCut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.ctrl {
            Some(true) => write!(f, "Ctrl + ")?,
            Some(false) => write!(f, "!Ctrl + ")?,
            None => (),
        }
        match self.shift {
            Some(true) => write!(f, "Shift + ")?,
            Some(false) => write!(f, "!Shift + ")?,
            None => (),
        }
        match self.alt {
            Some(true) => write!(f, "Alt + ")?,
            Some(false) => write!(f, "!Alt + ")?,
            None => (),
        }
        match self.tab {
            Some(true) => write!(f, "Tab + ")?,
            Some(false) => write!(f, "!Tab + ")?,
            None => (),
        }
        match self.windows {
            Some(true) => write!(f, "Windows + ")?,
            Some(false) => write!(f, "!Windows + ")?,
            None => (),
        }
        match self.mouse_l_button {
            Some(true) => write!(f, "MouseLeft + ")?,
            Some(false) => write!(f, "!MouseLeft + ")?,
            None => (),
        }
        match self.mouse_r_button {
            Some(true) => write!(f, "MouseRight + ")?,
            Some(false) => write!(f, "!MouseRight + ")?,
            None => (),
        }
        match self.mouse_m_button {
            Some(true) => write!(f, "MouseMiddle + ")?,
            Some(false) => write!(f, "!MouseMiddle + ")?,
            None => (),
        }
        match self.trigger_l {
            Some(v) => write!(f, "TriggerL({}) + ", v)?,
            None => (),
        }
        match self.trigger_r {
            Some(v) => write!(f, "TriggerR({}) + ", v)?,
            None => (),
        }
        match self.key {
            Some(Key(v)) => write!(f, " {:?}", v)?,
            None => (),
        }
        match self.controller_btn {
            Some((id, btn)) => write!(f, " ControllerBtn{btn}({id})")?,
            None => (),
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum AnyKey {
    /// Any key on the keyboard, Key is the key code
    Keyboard(Key),
    /// Any mouse button, u32 is the button code
    /// 0: left, 1: right, 2: middle
    MouseButton(u32),
    /// Any button on the controller, (u32, usize) is the controller id and button code
    Controller(u32, usize),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum AnyOffset {
    /// Any offset on the mouse, f64 is the offset value, respectively x and y
    Mouse(f64, f64),
    /// Any offset on the mouse wheel, f64 is the offset value, respectively x and y
    Wheel(f64, f64),
    /// Any offset on the controller, (u32, f64, f64) is the controller id and offsets
    /// respectively LeftTrigger, RightTrigger
    Trigger(u32, f64, f64),
    /// Any offset on the controller, (u32, f64, f64) is the controller id and offset x, y
    LeftStick(u32, f64, f64),
    /// Any offset on the controller, (u32, f64, f64) is the controller id and offset x, y
    RightStick(u32, f64, f64),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct AllOffsets {
    pub mouse: (f64, f64),
    pub wheel: (f64, f64),
    pub trigger: [(f64, f64); 4],
    pub left_stick: [(f64, f64); 4],
    pub right_stick: [(f64, f64); 4],
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum ControllerEvent {
    ButtonPress(usize),
    ButtonRelease(usize),
    TriggerMove(f64, f64),
    LSticksMove(f64, f64),
    RSticksMove(f64, f64),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Default)]
pub struct ControllerRaw {
    pub pack_num: u32,
    pub button: u16,
    pub tri: (u8, u8),
    pub sticker: (i16, i16, i16, i16),
}

impl ControllerRaw {
    pub fn trigger_change(&mut self, tri_l: u8, tri_r: u8) -> ControllerEvent {
        self.tri = (tri_l, tri_r);
        ControllerEvent::TriggerMove(tri_l as f64 / u8::MAX as f64, tri_r as f64 / u8::MAX as f64)
    }
    pub fn sl_change(&mut self, l_x: i16, l_y: i16) -> ControllerEvent {
        self.sticker.0 = l_x;
        self.sticker.1 = l_y;
        ControllerEvent::LSticksMove(l_x as f64 / i16::MAX as f64, l_y as f64 / i16::MAX as f64)
    }
    pub fn sr_change(&mut self, r_x: i16, r_y: i16) -> ControllerEvent {
        self.sticker.2 = r_x;
        self.sticker.3 = r_y;
        ControllerEvent::RSticksMove(r_x as f64 / i16::MAX as f64, r_y as f64 / i16::MAX as f64)
    }
    pub fn btn_change(&mut self, mut btn: u16) -> Vec<ControllerEvent> {
        let mut old = self.button;
        self.button = btn;
        let mut res = Vec::new();
        let mut index = 1;
        while btn != old {
            if btn & 1 != old & 1 {
                if old & 1 == 0 {
                    res.push(ControllerEvent::ButtonPress(index));
                } else {
                    res.push(ControllerEvent::ButtonRelease(index));
                }
            }
            index <<= 1;
            btn >>= 1;
            old >>= 1;
        }
        res
    }
}
