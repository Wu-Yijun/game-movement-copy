use std::{sync::mpsc::Receiver, thread::JoinHandle};

use log::{debug, info, warn};

use crate::{
    player::RecordPlayer,
    state::{AnyKey, AnyOffset, ControllerEvent, ControllerRaw, GlobalState, ShortCut, ShortCuts},
};

use rusty_xinput::XInputHandle;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Config {
    pub interval: f64,
    pub enable_mouse: bool,
    pub enable_keyboard: bool,
    pub enable_controller: [bool; 4],

    pub start_record: ShortCuts,
    pub append_record: ShortCuts,
    pub stop_record: ShortCuts,
    pub start_playback: ShortCuts,
    pub stop_playback: ShortCuts,
    pub continue_record: ShortCuts,
    pub drop_record: ShortCuts,

    pub save_records: ShortCuts,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval: 10.0,
            enable_mouse: true,
            enable_keyboard: true,
            enable_controller: [true, false, false, false],

            start_record: ShortCuts::Contains(vec![]),
            append_record: ShortCuts::Contains(vec![]),
            stop_record: ShortCuts::Contains(vec![]),
            start_playback: ShortCuts::Contains(vec![]),
            stop_playback: ShortCuts::Contains(vec![]),
            continue_record: ShortCuts::Contains(vec![]),
            drop_record: ShortCuts::Contains(vec![]),
            save_records: ShortCuts::Contains(vec![]),
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
            start_record: ShortCuts::Contains(vec![ShortCut::SHIFT_ENTER]),
            append_record: ShortCuts::Contains(vec![ShortCut::CTRL_SHIFT_ENTER]),
            stop_record: ShortCuts::Contains(vec![ShortCut::ESCAPE]),
            drop_record: ShortCuts::Contains(vec![ShortCut::SHIFT_ESCAPE]),
            start_playback: ShortCuts::Contains(vec![ShortCut::CTRL_ENTER]),
            stop_playback: ShortCuts::Contains(vec![ShortCut::ESCAPE]),
            continue_record: ShortCuts::Exclude(vec![ShortCut::EMPTY, ShortCut::ESCAPE]),

            save_records: ShortCuts::Contains(vec![ShortCut::CTRL_RIGHT_S]),
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

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Default)]
pub enum RecorderState {
    #[default]
    Ready,
    Recording,
    Playing,
    Error,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Recorder {
    config: Config,
    init_state: GlobalState,
    records: Vec<RecordEntry>,

    #[serde(skip)]
    player: RecordPlayer,

    #[serde(skip)]
    recorder: GlobalState,
    #[serde(skip)]
    /// rec start pos, old length, rec length
    rec_pos: (usize, usize, usize),
    #[serde(skip)]
    rdev_thread: Option<JoinHandle<()>>,
    #[serde(skip)]
    controller_thread: Option<JoinHandle<()>>,
    #[serde(skip)]
    recv: Option<Receiver<CallbackType>>,

    #[serde(skip)]
    pub state: RecorderState,
}
impl Default for Recorder {
    fn default() -> Self {
        Self {
            config: Config::new(),
            init_state: Default::default(),
            records: Vec::new(),
            player: RecordPlayer::new(),
            recorder: Default::default(),
            rec_pos: (0, 0, 0),
            rdev_thread: None,
            controller_thread: None,
            recv: None,
            state: RecorderState::Error,
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
        warn!("Save to file {path}!");
        let s = serde_yml::to_string(&self).unwrap();
        std::fs::write(path, s).unwrap();
    }

    pub fn init(&mut self) {
        self.state = RecorderState::Ready;

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

        self.player.init();
    }

    pub fn listen(&mut self) {
        let r = self.recv.as_ref().unwrap();
        match r.recv() {
            Ok(CallbackType::MK(ms, ev, s)) => {
                info!("MK:ms={:.2}\ts={:?}\t{:?}", ms, s, ev);
                if ms > self.recorder.time_ms + 1.0 {
                    self.next_ms(ms);
                }
                match ev {
                    rdev::EventType::KeyPress(key) => self.recorder.key_down(key.into()),
                    rdev::EventType::KeyRelease(key) => self.recorder.key_up(key.into()),
                    rdev::EventType::ButtonPress(button) => self.recorder.key_down(button.into()),
                    rdev::EventType::ButtonRelease(button) => self.recorder.key_up(button.into()),
                    rdev::EventType::MouseMove { x, y } => {
                        self.recorder.moves(AnyOffset::Mouse(x, y))
                    }
                    rdev::EventType::Wheel {
                        delta_x: x,
                        delta_y: y,
                    } => self.recorder.moves(AnyOffset::Wheel(x as f64, y as f64)),
                }
            }
            Ok(CallbackType::Ctrl(ms, id, ev)) => {
                info!("C{id}:ms={:.2}\t{:?}", ms, ev);
                if ms > self.recorder.time_ms + 1.0 {
                    self.next_ms(ms);
                }
                match ev {
                    ControllerEvent::ButtonPress(index) => {
                        self.recorder.key_down((id, index).into())
                    }
                    ControllerEvent::ButtonRelease(index) => {
                        self.recorder.key_up((id, index).into())
                    }
                    ControllerEvent::TriggerMove(x, y) => {
                        self.recorder.moves(AnyOffset::Trigger(id, x, y))
                    }
                    ControllerEvent::LSticksMove(x, y) => {
                        self.recorder.moves(AnyOffset::LeftStick(id, x, y))
                    }
                    ControllerEvent::RSticksMove(x, y) => {
                        self.recorder.moves(AnyOffset::RightStick(id, x, y))
                    }
                }
            }
            Err(e) => panic!("Receiver Error! {e}"),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.state != RecorderState::Error
    }

    fn next_ms(&mut self, ms: f64) {
        let e = self.recorder.next_ms(ms);
        if self.state == RecorderState::Recording {
            self.records.push(e);
        }
    }

    pub fn match_shortcuts(&mut self) -> RecorderState {
        let pat = self.recorder.get_pattern();
        debug!("Pattern: {:?}", pat);
        debug!("Pressed: {:?}", self.recorder.pressed_keys);
        if self
            .recorder
            .match_shortcuts(&pat, &self.config.save_records)
        {
            self.save_to_file("config.yaml".to_string());
        }
        match self.state {
            RecorderState::Ready => {
                if self
                    .recorder
                    .match_shortcuts(&pat, &self.config.append_record)
                {
                    info!("Append Rec.");
                    self.start_record(self.records.len())
                } else if self
                    .recorder
                    .match_shortcuts(&pat, &self.config.start_record)
                {
                    info!("New Rec.");
                    self.start_record(0)
                } else if self
                    .recorder
                    .match_shortcuts(&pat, &self.config.start_playback)
                {
                    self.start_playback()
                }
            }
            RecorderState::Recording => {
                if self
                    .recorder
                    .match_shortcuts(&pat, &self.config.drop_record)
                {
                    self.stop_record(true)
                } else if self
                    .recorder
                    .match_shortcuts(&pat, &self.config.stop_record)
                {
                    self.stop_record(false)
                }
            }
            RecorderState::Playing => {
                if self.player.is_done() {
                    self.stop_playback();
                } else if self
                    .recorder
                    .match_shortcuts(&pat, &self.config.continue_record)
                {
                    self.start_record(self.player.get_progress());
                    self.stop_playback();
                } else if self
                    .recorder
                    .match_shortcuts(&pat, &self.config.stop_playback)
                {
                    self.stop_playback();
                }
            }
            RecorderState::Error => (),
        }
        // self.current.match_shortcut(pat, shortcut)
        self.state.clone()
    }
}

impl Recorder {
    fn start_record(&mut self, continue_at: usize) {
        warn!("Start Recording!!! Continued at:{}", continue_at);
        if continue_at == 0 {
            self.rec_pos = (0, self.records.len(), 0);
            self.recorder.start_rec(0.0);
        } else {
            self.rec_pos = (continue_at, self.records.len(), 0);
            self.recorder.start_rec(self.records[continue_at - 1].ms);
        }
        info!("Recorder pos: {:?}", self.rec_pos);
        self.state = RecorderState::Recording;
    }
    fn stop_record(&mut self, discard_records: bool) {
        warn!("Stop Recording!!! Discard:{}", discard_records);
        let mut rec = self.records.split_off(self.rec_pos.1);
        info!("Records length: {}", rec.len());
        if !discard_records {
            if self.rec_pos.0 == 0 {
                self.records = rec;
                info!("Records replaced with rec.");
            } else {
                let _ = self.records.split_off(self.rec_pos.0);
                self.records.append(&mut rec);
                info!("Records cut at rec.");
            }
        }
        self.state = RecorderState::Ready;
    }
    fn start_playback(&mut self) {
        warn!("Start Playback!!!");
        self.player.start_playback(&self.records);
        self.state = RecorderState::Playing;
    }
    fn stop_playback(&mut self) {
        warn!("Stop Playback!!!");
        self.player.stop_playback();
        self.state = RecorderState::Ready;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RecordEntry {
    pub ms: f64,
    pub pressed: Vec<AnyKey>,
    pub released: Vec<AnyKey>,
    pub moves: Vec<AnyOffset>,
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

#[test]
fn test_shortcuts() {
    env_logger::builder()
        .target(env_logger::Target::Stdout)
        .filter_level(log::LevelFilter::Trace)
        .is_test(true)
        .init();
    let mut record = Recorder::from_file("".to_string());
    // record.state = RecorderState::Ready;
    // record.current.pressed_keys.push(rdev::Key::Return.into());
    // record.match_shortcuts();
    record.state = RecorderState::Playing;
    record.recorder.pressed_keys.push(rdev::Key::UpArrow.into());
    record.match_shortcuts();

    // panic!()
}

#[test]
fn test_player() {
    env_logger::builder()
        .target(env_logger::Target::Stdout)
        .filter_level(log::LevelFilter::Trace)
        .is_test(true)
        .init();

    let mut record = Recorder::from_file("config.yaml".to_string());
    println!("records length: {}", record.records.len());
    record.player.init();
    record.state = RecorderState::Ready;
    record
        .recorder
        .pressed_keys
        .push(rdev::Key::ControlLeft.into());
    record.recorder.pressed_keys.push(rdev::Key::Return.into());
    record.match_shortcuts();

    std::thread::sleep(std::time::Duration::from_secs(2));
}
