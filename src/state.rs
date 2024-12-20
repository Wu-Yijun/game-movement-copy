use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
};

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
    time_ms: f64,
    pressed_keys: Vec<AnyKey>,
    offsets: AllOffsets,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
// struct Key(u32);
struct Key(rdev::Key);

#[derive(Serialize, Deserialize, PartialEq)]
pub struct ShortCut {
    /// The key that triggers the shortcut.
    key: Option<Key>,
    /// The controller button that triggers the shortcut. (id, button)
    controller_btn: Option<(u32, u32)>,
    // The following modifiers are optional because they are not always needed.
    ctrl: Option<bool>,
    alt: Option<bool>,
    shift: Option<bool>,
    tab: Option<bool>,
    windows: Option<bool>,
    mouse_l_button: Option<bool>,
    mouse_r_button: Option<bool>,
    mouse_m_button: Option<bool>,
    // trigger on the stick of the id'th controller
    trigger_l: Option<u32>,
    trigger_r: Option<u32>,
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
    pub sticks: [(f64, f64, f64, f64); 4],
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum ControllerEvent {
    ButtonPress(usize),
    ButtonRelease(usize),
    TriggerMove(f64, f64),
    SticksMove(f64, f64, f64, f64),
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
        ControllerEvent::SticksMove(
            l_x as f64 / i16::MAX as f64,
            l_y as f64 / i16::MAX as f64,
            self.sticker.2 as f64 / i16::MAX as f64,
            self.sticker.3 as f64 / i16::MAX as f64,
        )
    }
    pub fn sr_change(&mut self, r_x: i16, r_y: i16) -> ControllerEvent {
        self.sticker.2 = r_x;
        self.sticker.3 = r_y;
        ControllerEvent::SticksMove(
            self.sticker.0 as f64 / i16::MAX as f64,
            self.sticker.1 as f64 / i16::MAX as f64,
            r_x as f64 / i16::MAX as f64,
            r_y as f64 / i16::MAX as f64,
        )
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
