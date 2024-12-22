use crate::recorder::RecordEntry;
use crate::state::{AnyKey, AnyOffset};
use log::{debug, warn};
use rdev::EventType;
use std::{
    sync::mpsc::{Receiver, Sender, TryRecvError},
    sync::{Arc, RwLock},
    thread::JoinHandle,
};

enum PlayerEvent {
    Start,
    Stop,
    Seek(usize),
    Update(Vec<RecordEntry>),
}

#[derive(Debug, Default)]
pub struct RecordPlayer {
    pub current_pos: Arc<RwLock<usize>>,
    pub is_playing: Arc<RwLock<bool>>,

    sender: Option<Sender<PlayerEvent>>,
    player: Option<JoinHandle<()>>,
}

impl RecordPlayer {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn init(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.sender = Some(tx);

        // Connect to the ViGEmBus driver
        let client = vigem_client::Client::connect().unwrap();
        // Create the virtual controller target
        let id = vigem_client::TargetId::XBOX360_WIRED;
        let mut target = vigem_client::Xbox360Wired::new(client, id);
        // Plugin the virtual controller
        target.plugin().unwrap();
        // Wait for the virtual controller to be ready to accept updates
        target.wait_ready().unwrap();

        let mut player = Player {
            recv: rx,
            is_playing: self.is_playing.clone(),
            current_pos: self.current_pos.clone(),
            records: Vec::new(),
            timer: std::time::Instant::now(),
            start_time: 0.0,
            controller: Controller::new(target),
        };
        let th = std::thread::spawn(move || {
            player.cycle();
        });
        self.player = Some(th);
    }
    pub fn get_progress(&self) -> usize {
        *self.current_pos.read().unwrap()
    }
    pub fn set_progress(&mut self, pos: usize) {
        let sender = self.sender.as_ref().unwrap();
        sender.send(PlayerEvent::Seek(pos)).unwrap();
    }
    pub fn is_done(&self) -> bool {
        !*self.is_playing.read().unwrap()
    }
    pub fn start_playback(&mut self, records: &[RecordEntry]) {
        let sender = self.sender.as_ref().unwrap();
        sender.send(PlayerEvent::Update(records.to_vec())).unwrap();
        sender.send(PlayerEvent::Start).unwrap();
        // TODO: have to manually change this because of timing problems.
        *self.is_playing.write().unwrap() = true;
    }
    pub fn stop_playback(&mut self) {
        let sender = self.sender.as_ref().unwrap();
        sender.send(PlayerEvent::Stop).unwrap();
    }
}

/// private
struct Player {
    recv: Receiver<PlayerEvent>,
    is_playing: Arc<RwLock<bool>>,
    current_pos: Arc<RwLock<usize>>,
    records: Vec<RecordEntry>,
    timer: std::time::Instant,

    start_time: f64,

    controller: Controller,
}

impl Player {
    fn cycle(&mut self) {
        loop {
            // process messages until empty
            let Some(is_empty) = self.process_msg() else {
                break;
            };
            if !is_empty {
                continue;
            }
            // check if playing, if not, wait for next message
            if !*self.is_playing.read().unwrap() {
                // wait for next message in 60fps
                std::thread::sleep(std::time::Duration::from_millis(1000 / 60));
                continue;
            }
            // try get the record at current position to play
            let pos = *self.current_pos.read().unwrap();
            let Some(record) = self.records.get(pos) else {
                self.stop();
                continue;
            };
            // sleep until next record time
            let ms = self.timer.elapsed().as_secs_f64() * 1000.0 - self.start_time;
            let dt = record.ms - ms;
            std::thread::sleep(std::time::Duration::from_secs_f64(dt.max(0.1) / 1000.0));
            // play the record
            self.play(pos);
            // move pos to next
            *self.current_pos.write().unwrap() = pos + 1;
            if pos + 1 >= self.records.len() {
                self.stop();
            }
        }
        self.stop();
    }

    fn start(&mut self) {
        warn!(
            "Player start at pos: {:?}",
            *self.current_pos.read().unwrap()
        );
        *self.is_playing.write().unwrap() = true;
        self.start_time = self.timer.elapsed().as_secs_f64() * 1000.0;
    }
    fn stop(&mut self) {
        warn!(
            "Player stops at pos: {:?}",
            *self.current_pos.read().unwrap()
        );
        *self.is_playing.write().unwrap() = false;
    }
    fn seek(&mut self, pos: usize) {
        warn!("Player pos seeks to: {:?}", pos);
        *self.current_pos.write().unwrap() = pos;
    }
    fn update(&mut self, records: Vec<RecordEntry>) {
        warn!("Player set records: {:?}", records.len());
        self.records = records;
        self.seek(0);
    }
    fn process_msg(&mut self) -> Option<bool> {
        match self.recv.try_recv() {
            Ok(PlayerEvent::Start) => self.start(),
            Ok(PlayerEvent::Stop) => self.stop(),
            Ok(PlayerEvent::Seek(pos)) => self.seek(pos),
            Ok(PlayerEvent::Update(records)) => self.update(records),
            Err(TryRecvError::Empty) => return Some(true), // nothing, continue playing
            Err(TryRecvError::Disconnected) => return None, // stop playing
        }
        Some(false) // maybe not empty
    }

    fn play(&mut self, pos: usize) {
        let record = &self.records[pos];
        let mut controller = &mut self.controller;
        // play the record
        for key in &record.pressed {
            Self::press(key, &mut controller).unwrap();
        }
        for key in &record.released {
            Self::release(key, &mut controller).unwrap();
        }
        for offset in &record.moves {
            Self::moves(offset, &mut controller).unwrap();
        }
        self.controller.try_update();
    }
    fn to_btn(btn: u32, press: bool) -> EventType {
        let btn = match btn {
            0 => rdev::Button::Left,
            1 => rdev::Button::Middle,
            2 => rdev::Button::Right,
            i => rdev::Button::Unknown(i as u8),
        };
        if press {
            EventType::ButtonPress(btn)
        } else {
            EventType::ButtonRelease(btn)
        }
    }
    fn press(key: &AnyKey, controller: &mut Controller) -> Result<(), rdev::SimulateError> {
        debug!("press: {:?}", key);
        match key {
            AnyKey::Keyboard(key) => rdev::simulate(&key.press()),
            AnyKey::MouseButton(btn) => rdev::simulate(&Self::to_btn(*btn, true)),
            AnyKey::Controller(_, code) => Ok(controller.press(*code as u16)),
        }
    }
    fn release(key: &AnyKey, controller: &mut Controller) -> Result<(), rdev::SimulateError> {
        debug!("release: {:?}", key);
        match key {
            AnyKey::Keyboard(key) => rdev::simulate(&key.release()),
            AnyKey::MouseButton(btn) => rdev::simulate(&Self::to_btn(*btn, false)),
            AnyKey::Controller(_, code) => Ok(controller.release(*code as u16)),
        }
    }
    fn moves(offset: &AnyOffset, controller: &mut Controller) -> Result<(), rdev::SimulateError> {
        debug!("move: {:?}", offset);
        match *offset {
            AnyOffset::Mouse(x, y) => rdev::simulate(&EventType::MouseMove { x, y }),
            AnyOffset::Wheel(dx, dy) => rdev::simulate(&EventType::Wheel {
                delta_x: dx as i64,
                delta_y: dy as i64,
            }),
            AnyOffset::Trigger(_, l, r) => Ok(controller.trigger(l, r)),
            AnyOffset::LeftStick(_, x, y) => Ok(controller.left_stick(x, y)),
            AnyOffset::RightStick(_, x, y) => Ok(controller.right_stick(x, y)),
        }
    }
}

#[derive(Debug)]
struct Controller {
    // client: vigem_client::Client,
    target: vigem_client::Xbox360Wired<vigem_client::Client>,
    gamepad: vigem_client::XGamepad,
    updated: bool,
}

impl Controller {
    fn new(target: vigem_client::Xbox360Wired<vigem_client::Client>) -> Self {
        Self {
            target,
            gamepad: Default::default(),
            updated: true,
        }
    }

    fn try_update(&mut self) {
        if self.updated {
            self.updated = false;
            self.target.update(&self.gamepad).unwrap();
        }
    }

    fn press(&mut self, btn: u16) {
        if self.gamepad.buttons.raw & btn == 0 {
            self.updated = true;
            self.gamepad.buttons.raw ^= btn;
        }
    }
    fn release(&mut self, btn: u16) {
        if self.gamepad.buttons.raw & btn != 0 {
            self.updated = true;
            self.gamepad.buttons.raw ^= btn;
        }
    }
    fn trigger(&mut self, l: f64, r: f64) {
        let l = (l * u8::MAX as f64).round() as u8;
        let r = (r * u8::MAX as f64).round() as u8;
        if self.gamepad.left_trigger != l || self.gamepad.right_trigger != r {
            self.updated = true;
            self.gamepad.left_trigger = l;
            self.gamepad.right_trigger = r;
        }
    }
    fn left_stick(&mut self, x: f64, y: f64) {
        let x = (x * i16::MAX as f64).round() as i16;
        let y = (y * i16::MAX as f64).round() as i16;
        if self.gamepad.thumb_lx != x || self.gamepad.thumb_ly != y {
            self.updated = true;
            self.gamepad.thumb_lx = x;
            self.gamepad.thumb_ly = y;
        }
    }
    fn right_stick(&mut self, x: f64, y: f64) {
        let x = (x * i16::MAX as f64).round() as i16;
        let y = (y * i16::MAX as f64).round() as i16;
        if self.gamepad.thumb_rx != x || self.gamepad.thumb_ry != y {
            self.updated = true;
            self.gamepad.thumb_rx = x;
            self.gamepad.thumb_ry = y;
        }
    }
}
