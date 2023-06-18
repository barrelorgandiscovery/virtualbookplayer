use std::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
    time::Instant,
};

use bookparsing::{Hole, VirtualBook};
use player::{Command, Note, Player, Response};

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

    pub start_play_time: Instant,
    /// virtual book
    pub virtual_book: Option<Arc<VirtualBook>>,
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
            virtual_book: None,
            start_play_time: Instant::now() - Duration::from_millis(10_000),
        }
    }

    pub fn player(&mut self, player: Option<(Box<dyn Player>, Receiver<Response>)>) {
        if let Some(old_player_mutex) = &self.player {
            let mut old_player = old_player_mutex.lock().unwrap();
            old_player.stop();
            drop(old_player);
        }

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

            if !self.playlist.file_list.is_empty() {
                if let Some(n) = self.playlist.file_list.get(0) {
                    self.start_play_time = Instant::now(); // before play
                    if let Err(e) = p.play(&n.path) {
                        error!("error in playing file : {}", e);
                    } else {
                    }
                }
            }
        }
        let notes = self.notes();

        let mut virt = VirtualBook::midi_scale();
        virt.holes.holes = notes
            .iter()
            .map(|n| Hole {
                timestamp: u64::try_from(n.start.as_micros()).unwrap(),
                length: u64::try_from(n.length.as_micros()).unwrap(),
                track: (127 - n.note).into(),
            })
            .collect();

        self.virtual_book = Some(Arc::new(virt));
    }

    #[allow(dead_code)]
    pub fn start_play_time(&self) -> Instant {
        self.start_play_time
    }

    /// get visual notes of the current played file
    pub fn notes(&self) -> Arc<Vec<Note>> {
        if let Some(player) = &self.player {
            let p = player.lock().unwrap();
            return p.associated_notes();
        }
        Arc::new(vec![])
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
