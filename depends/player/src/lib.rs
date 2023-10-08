use std::{
    error::Error,
    path::PathBuf,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
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
#[derive(Clone, Debug)]
pub struct FileInformations {
    pub duration: Option<Duration>,
}

pub trait PlayerFactory {
    /// create a factory for player instanciation
    fn create(
        &self,
        sender: Sender<Response>,
        receiver: Receiver<Command>,
    ) -> Result<Box<dyn Player>, Box<dyn Error>>;

    /// get the information associated to a given file,
    /// return the associated informations
    fn create_information_getter(
        &self,
    ) -> Result<Box<dyn FileInformationsConstructor>, Box<dyn Error>>;
}

pub trait Player: Send {
    fn start_play(
        &mut self,
        filename: &PathBuf,
        start_time: Option<f32>,
    ) -> Result<(), Box<dyn Error>>;
    fn stop(&mut self);
    fn is_playing(&self) -> bool;

    // in milliseconds
    fn current_play_time(&self) -> i64;
    fn associated_notes(&self) -> Arc<Mutex<Arc<Vec<Note>>>>;
}

pub trait FileInformationsConstructor: Send {
    fn compute(&mut self, filename: &PathBuf) -> Result<Arc<FileInformations>, Box<dyn Error>>;
}

#[derive(Debug)]
pub enum Response {
    EndOfFile,
    FileCancelled,
    CurrentPlayTime(Duration),
    FilePlayStarted((String, Arc<Vec<Note>>)),
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
