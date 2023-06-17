use std::{
    error::Error,
    path::PathBuf,
    sync::{
        mpsc::{Receiver, Sender},
        Arc,
    },
    time::Duration,
};

pub mod midiio;

#[derive(Debug)]
pub struct Note {
    pub channel: u16,
    pub note: u8,
    pub start: Duration,
    pub length: Duration,
}

pub struct FileInformations {
    pub duration: Option<Duration>
}


pub trait PlayerFactory {
    fn create(
        &self,
        sender: Sender<Response>,
        receiver: Receiver<Command>,
    ) -> Result<Box<dyn Player>, Box<dyn Error>>;

    fn create_information_getter(&self) -> Result<Box<dyn FileInformationsConstructor>, Box<dyn Error>>;
}

pub trait Player: Send {
    fn play(&mut self, filename: &PathBuf) -> Result<(), Box<dyn Error>>;
    fn stop(&mut self);
    fn is_playing(&self) -> bool;
    // in milliseconds
    fn current_play_time(&self) -> i64;
    fn associated_notes(&self) -> Arc<Vec<Note>>;
}

pub trait FileInformationsConstructor: Send {
    fn compute(self, filename: &PathBuf) -> Result<FileInformations,Box<dyn Error>>;
}

#[derive(Debug)]
pub enum Response {
    EndOfTrack,
    StartOfTrack,
    EndOfFile,
    FileCancelled,
    CurrentPlayTime(Duration),
}

#[derive(Debug)]
pub enum Command {
    Replay,
    Silence,
    Reset,
    Solo,
    Info,
    /// Changes the speed by the value given.
    Speed(f32),
    // NoteStyle,
}
