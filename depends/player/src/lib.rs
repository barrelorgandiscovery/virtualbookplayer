#![crate_name = "player"]

//! # Player Crate
//! This crate provide an abstraction to a player, with simple functions working on file as a whole.
//!
//! in the current implementation, two file types reading are provided, midi or book
//!
//!

use std::{
    error::Error,
    path::PathBuf,
    sync::{
        mpsc::{Receiver, Sender},
        Arc,
    },
    time::Duration,
};

/// midiio module handle the midi output for playing
pub mod midiio;

/// this structure handle a plain note (with start and length)
#[derive(Debug)]
pub struct PlainNoteWithChannel {
    pub channel: u16,
    pub note: u8,
    pub track: u16,
    pub start: Duration,
    pub length: Duration,
}

/// this structure provide additional informations on files (using in the gui to display duration and additional useful informations on the file)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInformations {
    pub duration: Option<Duration>,
}

/// Factory for the player, with a command and responses to and from the player
/// commands and response permit to communicate with the player which has it's own
/// execution thread
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

#[derive(Debug)]
pub struct NotesDisplayInformations {
    pub width: f32,
    pub first_axis: f32,
    pub inter_axis: f32,
    pub track_width: f32,
    pub preferred_view_inversed: bool,
}

#[derive(Debug)]
pub struct NotesInformations {
    pub notes: Arc<Vec<PlainNoteWithChannel>>,
    pub display_informations: NotesDisplayInformations,
}

impl Default for NotesInformations {
    fn default() -> Self {
        NotesInformations {
            notes: Arc::new(vec![]),
            display_informations: NotesDisplayInformations {
                width: 128.0,
                first_axis: 1.0,
                inter_axis: 1.0,
                track_width: 1.0,
                preferred_view_inversed: true,
            },
        }
    }
}

/// Player trait
pub trait Player: Send {
    /// Start playing a file, in asynchronous manner
    /// once the play is started the player send some response to inform :
    ///
    /// the current timestamp played in the file
    ///
    fn start_play(
        &mut self,
        filename: &PathBuf,
        start_time: Option<f32>,
    ) -> Result<(), Box<dyn Error>>;

    /// Stop the current play
    fn stop(&mut self);

    // is in pause ?
    fn is_paused(&self) -> bool;

    /// inform if the player is in state "playing"
    fn is_playing(&self) -> bool;

    /// get the current play time, in milliseconds
    fn current_play_time(&self) -> i64;

    /// grab a copy of the notes of the current file (for display)
    fn associated_notes(&self) -> Arc<NotesInformations>;

    /// get the information associated to a given file,
    /// return the associated informations
    fn create_information_getter(
        &self,
    ) -> Result<Box<dyn FileInformationsConstructor>, Box<dyn Error>>;
}

/// Factory for file information creator, using the compute function this compute the
/// associated information on a given file
pub trait FileInformationsConstructor: Send {
    /// compute additional information about a given file
    fn compute(&mut self, filename: &PathBuf) -> Result<FileInformations, Box<dyn Error>>;
}

/// messages from the player
#[derive(Debug)]
pub enum Response {
    EndOfFile,
    FileCancelled,
    CurrentPlayTime(Duration),
    FilePlayStarted((String, Arc<NotesInformations>)),
}

/// commands that can be sent to the player
#[derive(Debug)]
pub enum Command {
    Replay,
    Silence,
    Reset,
    Solo,
    Info,
    Pause,
    /// Changes the speed by the value given.
    Speed(f32),
}
