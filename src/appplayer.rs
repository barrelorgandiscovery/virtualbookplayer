use std::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use midir::MidiOutputConnection;
use nodi::{midly::live::SystemRealtime, Connection, MidiEvent};
use player::{Command, Player, Response};

use crate::playlist::PlayList;

use log::error;

///
/// player integrating the playlist, and play mod
///
pub struct AppPlayer {
    /// underlying player, depending on the technology used (midi or other)
    pub player: Option<Arc<Mutex<Box<dyn Player>>>>,

    /// play list containing the list of references of the files
    pub playlist: PlayList,

    /// command send to the player
    pub commands: Sender<Command>,

    pub last_response: Arc<Mutex<Option<Response>>>,

    /// play mod,
    pub play_mod: bool,
}

impl AppPlayer {
    pub fn new() -> AppPlayer {
        let commands = channel();

        AppPlayer {
            commands: commands.0,
            player: None,
            playlist: PlayList::new(),
            play_mod: false,
            last_response: Arc::new(Mutex::new(None)),
        }
    }

    pub fn player(&mut self, player: Option<(Box<dyn Player>, Receiver<Response>)>) {
        self.player = match player {
            None => None,
            Some(p) => {
                let player_reference = Arc::new(Mutex::new(p.0));
                let last_response = Arc::clone(&self.last_response);

                thread::spawn(move || {
                    // println!("start thread for getting responses");
                    while let Ok(response) = p.1.recv() {
                        // println!("received a response from inner player : {:?}", response);
                        if let Ok(mut m) = last_response.lock() {
                            *m = Some(response);
                        }
                    }
                });

                Some(player_reference)
            }
        };
    }

    pub fn play_file_on_top(&mut self) {
        if let Some(player) = &self.player {
            let mut p = player.lock().unwrap();
            p.stop();

            if self.playlist.file_list.len() > 0 {
                if let Some(n) = self.playlist.file_list.get(0) {
                    let f = n.borrow();
                    if let Err(e) = p.play(&f.path) {
                        error!("error in playing file : {}", e);
                    }
                }
            }
            return;
        }
    }

    /// stop the play
    pub fn stop(&mut self) {
        if let Some(player) = &self.player {
            let mut p = player.lock().unwrap();
            p.stop();
        }
    }

    /// next file, and "unpop" the currently played
    pub fn next(&mut self) {
        self.playlist.skip();
        if self.play_mod {
            self.play_file_on_top();
        }
    }

    pub fn is_playing(&self) -> bool {
        if let Some(player) = &self.player {
            let p = player.lock().unwrap();
            p.is_playing()
        } else {
            false
        }
    }
}
