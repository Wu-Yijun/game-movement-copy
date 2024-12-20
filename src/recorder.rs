use std::{sync::mpsc::Receiver, thread::JoinHandle};

use crate::state::{
    AnyKey, AnyOffset, ControllerEvent, ControllerRaw, GlobalState, ShortCut, ShortCuts,
};

use rusty_xinput::XInputHandle;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Config {
    pub interval: f64,
    pub enable_mouse: bool,
    pub enable_keyboard: bool,
    pub enable_controller: [bool; 4],

    pub start_record_button: ShortCuts,
    pub stop_record_button: ShortCuts,
    pub start_playback_button: ShortCuts,
    pub stop_playback_button: ShortCuts,
    pub continue_record_button: ShortCuts,
    pub drop_record_button: ShortCuts,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval: 10.0,
            enable_mouse: true,
            enable_keyboard: true,
            enable_controller: [true, false, false, false],

            start_record_button: ShortCuts::Contains(vec![]),
            stop_record_button: ShortCuts::Contains(vec![]),
            start_playback_button: ShortCuts::Contains(vec![]),
            stop_playback_button: ShortCuts::Contains(vec![]),
            continue_record_button: ShortCuts::Contains(vec![]),
            drop_record_button: ShortCuts::Contains(vec![]),
        }
    }
}

impl Config {
    /// default config
    /// Short Cut keys:
    /// - `Shift + Enter` to start recording
    /// - `Escape` to stop recording
    /// - `Ctrl + Enter` to start playback
    /// - `Escape` to stop playback
    /// - Any key except `Escape` to continue recording at the current time while playing back
    /// - `Shift + Escape` to drop the current recording, and to remain the last recording unchanged
    pub fn new() -> Self {
        Self {
            start_record_button: ShortCuts::Contains(vec![ShortCut::SHIFT_ENTER]),
            stop_record_button: ShortCuts::Contains(vec![ShortCut::ESCAPE]),
            drop_record_button: ShortCuts::Contains(vec![ShortCut::SHIFT_ESCAPE]),
            start_playback_button: ShortCuts::Contains(vec![ShortCut::CTRL_ENTER]),
            stop_playback_button: ShortCuts::Contains(vec![ShortCut::ESCAPE]),
            continue_record_button: ShortCuts::Exclude(vec![ShortCut::ESCAPE]),
            ..Default::default()
        }
    }
}

enum CallbackType {
    /// Mouse or Keyboard
    MK(f64, rdev::EventType, String),
    /// Controller
    Ctrl(f64, u32, ControllerEvent),
}
// type MkCallbackType = (f64, rdev::EventType, String);
// type CtCallbackType = (f64, u32, ControllerEvent);

#[derive(Serialize, Deserialize, Debug)]
pub struct Recorder {
    config: Config,
    init_state: GlobalState,
    records: Vec<RecordEntry>,

    #[serde(skip)]
    current: GlobalState,
    #[serde(skip)]
    rdev_thread: Option<JoinHandle<()>>,
    #[serde(skip)]
    controller_thread: Option<JoinHandle<()>>,
    #[serde(skip)]
    recv: Option<Receiver<CallbackType>>,
}
impl Default for Recorder {
    fn default() -> Self {
        Self {
            config: Config::new(),
            init_state: Default::default(),
            records: Vec::new(),
            current: Default::default(),
            rdev_thread: None,
            controller_thread: None,
            recv: None,
        }
    }
}

fn shake_all(handle: &XInputHandle) -> Vec<bool> {
    let res: Vec<_> = (0..4)
        .map(|i| handle.set_state(i, 40000, 40000).is_ok())
        .collect();
    std::thread::sleep(::std::time::Duration::from_millis(500));
    (0..4).for_each(|i| {
        let _ = handle.set_state(i, 0, 0);
    });
    res
}

impl Recorder {
    pub fn from_file(path: String) -> Self {
        std::fs::read_to_string(path).map_or_else(
            |_| Self::default(),
            |s| serde_yml::from_str(&s).unwrap_or_default(),
        )
    }
    pub fn save_to_file(&self, path: String) {
        let s = serde_yml::to_string(&self).unwrap();
        std::fs::write(path, s).unwrap();
    }

    pub fn init(&mut self) {
        // 创建一个用于发送的通道
        let (tx, rx) = std::sync::mpsc::channel::<CallbackType>();
        // 传递给闭包的起始时间点
        let start_time = std::time::Instant::now();

        // 键盘鼠标监听器
        if self.config.enable_keyboard || self.config.enable_mouse {
            let use_mouse = self.config.enable_mouse;
            let use_key = self.config.enable_keyboard;
            let tx = tx.clone();
            let th = std::thread::spawn(move || {
                // 假设这是我们要传递给闭包的起始时间点
                let start_time = start_time.clone();
                rdev::listen(move |e| {
                    // filter skip by enables
                    match e.event_type {
                        rdev::EventType::KeyPress(_) | rdev::EventType::KeyRelease(_) => {
                            if !use_key {
                                return;
                            }
                        }
                        rdev::EventType::ButtonPress(_)
                        | rdev::EventType::ButtonRelease(_)
                        | rdev::EventType::MouseMove { .. }
                        | rdev::EventType::Wheel { .. } => {
                            if !use_mouse {
                                return;
                            }
                        }
                    }
                    // send msg
                    let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;
                    let ev = CallbackType::MK(elapsed_ms, e.event_type, e.name.unwrap_or_default());
                    tx.send(ev).expect("Failed to send time");
                })
                .expect("Cannot create listener!");
            });
            self.rdev_thread.replace(th);
        }

        // 手柄监听器
        let uses: Vec<u32> = self
            .config
            .enable_controller
            .iter()
            .enumerate()
            .filter_map(|(i, b)| if *b { Some(i as u32) } else { None })
            .collect();
        if !uses.is_empty() {
            let interval = (self.config.interval * 1000.0) as u64;
            let th = std::thread::spawn(move || {
                let handle = XInputHandle::load_default().unwrap();
                // just to test
                let enabled = shake_all(&handle);
                println!("Connection State: {:?}", enabled);
                let mut controllers = vec![ControllerRaw::default(); 4];
                loop {
                    if interval > 0 {
                        std::thread::sleep(std::time::Duration::from_micros(interval));
                    }
                    let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;
                    for &i in uses.iter() {
                        if let Ok(state) = handle.get_state(i) {
                            let ctr = &mut controllers[i as usize];
                            if state.raw.dwPacketNumber == ctr.pack_num {
                                // not updated
                                continue;
                            }
                            let pad = &state.raw.Gamepad;
                            if pad.bLeftTrigger != ctr.tri.0 || pad.bRightTrigger != ctr.tri.1 {
                                let ev = ctr.trigger_change(pad.bLeftTrigger, pad.bRightTrigger);
                                tx.send(CallbackType::Ctrl(elapsed_ms, i, ev)).unwrap();
                            }
                            if pad.sThumbLX != ctr.sticker.0 || pad.sThumbLY != ctr.sticker.1 {
                                let ev = ctr.sl_change(pad.sThumbLX, pad.sThumbLY);
                                tx.send(CallbackType::Ctrl(elapsed_ms, i, ev)).unwrap();
                            }
                            if pad.sThumbRX != ctr.sticker.2 || pad.sThumbRY != ctr.sticker.3 {
                                let ev = ctr.sr_change(pad.sThumbRX, pad.sThumbRY);
                                tx.send(CallbackType::Ctrl(elapsed_ms, i, ev)).unwrap();
                            }
                            if pad.wButtons != ctr.button {
                                let actions = ctr.btn_change(pad.wButtons);
                                for ev in actions {
                                    tx.send(CallbackType::Ctrl(elapsed_ms, i, ev)).unwrap();
                                }
                            }
                        }
                    }
                }
            });
            self.controller_thread.replace(th);
        }
        self.recv.replace(rx);
    }

    pub fn listen(&mut self) {
        let r = self.recv.as_ref().unwrap();
        match r.recv() {
            Ok(CallbackType::MK(ms, ev, s)) => {
                println!("MK:ms={:.2}\t{:?}", ms, ev);
            }
            Ok(CallbackType::Ctrl(ms, id, ev)) => {
                println!("C{id}:ms={:.2}\t{:?}", ms, ev);
            }
            Err(e) => panic!("Receiver Error! {e}"),
        }
    }

    // fn write_mouse_key(&self, )
}

#[derive(Serialize, Deserialize, Debug)]
struct RecordEntry {
    ms: f64,
    pressed: Vec<AnyKey>,
    released: Vec<AnyKey>,
    moves: Vec<AnyOffset>,
}

#[test]
fn test_yaml() {
    let recorder = Recorder::from_file("config.yaml".to_string());
    println!("{:#?}", recorder);
    recorder.save_to_file("config.yaml".to_string());
}

#[test]
fn test_recorder() {
    let mut record = Recorder::from_file("config.yaml".to_string());
    record.init();
    loop {
        record.listen();
    }
}
