use std::{
    sync::{
        mpsc::{Receiver, Sender, TryRecvError},
        Arc, RwLock,
    },
    thread::JoinHandle,
};

use log::{debug, warn};

use crate::recorder::RecordEntry;

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

/// private
struct Player {
    recv: Receiver<PlayerEvent>,
    is_playing: Arc<RwLock<bool>>,
    current_pos: Arc<RwLock<usize>>,
    records: Vec<RecordEntry>,
    timer: std::time::Instant,

    start_time: f64,
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
            if dt > 0.5 {
                std::thread::sleep(std::time::Duration::from_secs_f64(dt / 1000.0));
            }
            // play the record
            self.play(record);
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

    fn play(&self, record: &RecordEntry) {
        // play the record
        for key in &record.pressed {
            // press the key
            debug!("press: {:?}", key);
        }
        for key in &record.released {
            // release the key
            debug!("release: {:?}", key);
        }
        for offset in &record.moves {
            // move the mouse
            debug!("move: {:?}", offset);
        }
    }
}

impl RecordPlayer {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn init(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.sender = Some(tx);
        let mut player = Player {
            recv: rx,
            is_playing: self.is_playing.clone(),
            current_pos: self.current_pos.clone(),
            records: Vec::new(),
            timer: std::time::Instant::now(),
            start_time: 0.0,
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
    }
    pub fn stop_playback(&mut self) {
        let sender = self.sender.as_ref().unwrap();
        // stop
        sender.send(PlayerEvent::Stop).unwrap();
        // reset
        sender.send(PlayerEvent::Seek(0)).unwrap();
    }
}
