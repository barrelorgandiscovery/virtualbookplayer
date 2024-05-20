//! Hold the Playlist and Player fusion to provide the gui a unique interface

use std::{
    collections::HashSet,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
    time::Instant,
};

use bookparsing::{Hole, VirtualBook};
use egui::mutex::RwLock;
use player::{Command, FileInformationsConstructor, NotesInformations, Player, Response};

use crate::{playlist::PlayList, virtualbookcomponent::IndexedVirtualBook};

use log::{debug, error, info, warn};

///
/// player integrating the playlist, and play mod
///
pub struct AppPlayer {
    /// underlying player, depending on the technology used (midi or other)
    pub player: Option<Arc<Mutex<Box<dyn Player>>>>,

    /// play list containing the list of references of the files
    pub playlist: Arc<Mutex<PlayList>>,

    /// command send to the player
    pub commands: Sender<Command>,

    pub last_response: Arc<Mutex<Option<Response>>>,

    /// play mod,
    pub play_mod: bool,

    pub start_play_time: Instant,

    /// virtual book
    pub virtual_book: Arc<RwLock<Option<Arc<IndexedVirtualBook>>>>,

    // starting time wait
    pub waittime_between_file_play: f32,

    // appplayer cmd sender
    applayer_sender: Sender<AppPlayerThreadCommands>,

    bgthread_sender: Sender<AppPlayerBackgroundThreadCommands>,
}

enum AppPlayerThreadCommands {
    /// signal notes have changed
    NotesChanged(Arc<NotesInformations>),
}

#[allow(unused)]
enum AppPlayerEvent {}

enum AppPlayerBackgroundThreadCommands {
    PlayerChanged(Arc<Mutex<Box<dyn Player>>>),
}

type PlayerTriplet = (Box<dyn Player>, Receiver<Response>, Sender<Command>);

/// manage asynchrone actions (play, informations retrieve)
impl AppPlayer {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let commands = channel();

        let inner_control_thread = channel::<AppPlayerThreadCommands>();

        let bg_thread_communication = channel::<AppPlayerBackgroundThreadCommands>();

        let appplayer = Self {
            commands: commands.0,
            player: None,
            playlist: Arc::new(Mutex::new(PlayList::new())),
            play_mod: false,
            last_response: Arc::new(Mutex::new(None)),
            virtual_book: Arc::new(RwLock::new(None)),
            start_play_time: Instant::now() - Duration::from_millis(10_000),
            waittime_between_file_play: 0_f32,
            applayer_sender: inner_control_thread.0,
            bgthread_sender: bg_thread_communication.0,
        };

        let vb_access = Arc::clone(&appplayer.virtual_book);
        thread::spawn(move || {
            #[cfg(feature = "profiling")]
            profiling::register_thread!("AppPlayerThreadCommands");
            let receiver = inner_control_thread.1;
            loop {
                while let Ok(cmd) = receiver.recv() {
                    match cmd {
                        AppPlayerThreadCommands::NotesChanged(notes) => {
                            let mut virt = VirtualBook::midi_scale();
                            virt.holes.holes = notes
                                .notes
                                .iter()
                                .map(|n| {
                                    let t = i64::try_from(n.start.as_micros());
                                    if t.is_err() {
                                        warn!(
                                            "error converting timestamp : {}",
                                            n.start.as_micros()
                                        );
                                    }
                                    let l = i64::try_from(n.length.as_micros());
                                    if l.is_err() {
                                        warn!("error converting length : {}", n.length.as_micros());
                                    }

                                    Hole {
                                        timestamp: t.unwrap(),
                                        length: l.unwrap(),
                                        track: n.track,
                                    }
                                })
                                .collect();

                            virt.scale.definition.width = notes.display_informations.width;
                            virt.scale.definition.defaulttrackheight =
                                notes.display_informations.track_width;
                            virt.scale.definition.firsttrackdistance =
                                notes.display_informations.first_axis;
                            virt.scale.definition.intertrackdistance =
                                notes.display_informations.inter_axis;
                            virt.scale.definition.ispreferredviewinverted =
                                notes.display_informations.preferred_view_inversed;

                            let mut wlock = vb_access.write();
                            *wlock = Some(Arc::new(IndexedVirtualBook::from(&Arc::new(virt))));
                        }
                    }
                }
            }
        });

        // create the inner
        let local_playlist = Arc::clone(&appplayer.playlist);

        thread::spawn(move || {
            #[cfg(feature = "profiling")]
            profiling::register_thread!("AppPlayer inner player command receiver");
            let mut current_player: Option<Arc<Mutex<Box<dyn Player>>>> = None;
            loop {
                {
                    if let Ok(command) = bg_thread_communication
                        .1
                        .recv_timeout(Duration::from_secs(2))
                    {
                        match command {
                            AppPlayerBackgroundThreadCommands::PlayerChanged(p) => {
                                current_player = Some(p);
                            }
                        }
                    } else if current_player.clone().is_none() {
                        info!("no current player, watch");
                        continue;
                    }
                }

                let mut playlist_copy = HashSet::new();
                // get list
                {
                    if let Ok(playlist) = local_playlist.lock() {
                        for p in &playlist.file_list {
                            playlist_copy.insert(p.clone());
                        }
                    } else {
                        error!("error in getting the playerlist lock");
                        continue;
                    }
                }

                let mut computed = HashSet::new();
                {
                    for p in &playlist_copy {
                        if p.additional_informations.is_none() {
                            // compute the additional informations
                            let mut local_info_getter: Option<
                                Box<dyn FileInformationsConstructor>,
                            > = None;

                            if let Ok(player) = &current_player.clone().unwrap().lock() {
                                if let Ok(info_getter) = player.create_information_getter() {
                                    local_info_getter = Some(info_getter);
                                }
                            } // this unlock the appplayer

                            if let Some(mut info_getter) = local_info_getter {
                                info!("compute informations for {}", &p.path.display());

                                let mut computed_p = p.clone();
                                if let Ok(result) = info_getter.compute(&p.path) {
                                    computed_p.additional_informations = Some(result.clone());
                                    computed.insert(computed_p);
                                }
                            }
                        }
                    }
                }

                // update list
                {
                    if let Ok(mut playlist) = local_playlist.lock() {
                        // play list to modify
                        for p in &mut playlist.file_list {
                            if p.additional_informations.is_none() {
                                // update
                                for e in &computed {
                                    if e.added_at == p.added_at {
                                        p.additional_informations =
                                            e.additional_informations.clone();
                                    }
                                }
                            }
                        }

                        let mut total_duration: Duration = Duration::ZERO;
                        // update the global time of the playlist
                        for p in &playlist.file_list {
                            if let Some(additional) = &p.additional_informations {
                                if let Some(length) = &additional.duration {
                                    total_duration += *length;
                                }
                            }
                        }

                        playlist.computed_length = Some(total_duration);
                    } else {
                        error!("error in getting the playerlist lock");
                        continue;
                    }
                }
            }
        });

        appplayer
    }

    pub fn set_waittime_between_file_play(&mut self, wait_time: f32) {
        self.waittime_between_file_play = wait_time;
    }

    /// define the current player, with associated receiver for the player response
    pub fn player(&mut self, player: Option<PlayerTriplet>) {
        if let Some(old_player_mutex) = &self.player {
            let mut old_player = old_player_mutex.lock().unwrap();
            old_player.stop();
            drop(old_player);
        }

        self.play_mod = false; // reset the automatic play when the player changed

        if let Some((private_player, private_command_sender)) = match player {
            None => None,
            Some(p) => {
                let player_reference = Arc::new(Mutex::new(p.0));
                let last_response = Arc::clone(&self.last_response);

                let inner_thread_access = self.applayer_sender.clone();
                // create inner control thread of the player
                thread::spawn(move || {
                    #[cfg(feature = "profiling")]
                    profiling::register_thread!("player commands receiver");
                    // println!("start thread for getting responses");
                    while let Ok(response) = p.1.recv() {
                        // println!("received a response from inner player : {:?}", response);

                        match &response {
                            Response::CurrentPlayTime(_time) => {}
                            Response::EndOfFile => {}
                            Response::FileCancelled => {}
                            Response::FilePlayStarted((_filename, notes)) => {
                                if let Err(e) = inner_thread_access
                                    .send(AppPlayerThreadCommands::NotesChanged(Arc::clone(notes)))
                                {
                                    error!(
                                        "error when sending notes changed for app player : {:?}",
                                        e
                                    );
                                }
                            }
                        }

                        // forward the response
                        if let Ok(mut m) = last_response.lock() {
                            *m = Some(response);
                        }
                    }
                });

                if let Err(e) =
                    self.bgthread_sender
                        .send(AppPlayerBackgroundThreadCommands::PlayerChanged(
                            Arc::clone(&player_reference),
                        ))
                {
                    debug!("fail to send the player to background thread :{}", e);
                }

                Some((player_reference, p.2))
            }
        } {
            self.player = Some(private_player);
            self.commands = private_command_sender;
        } else {
            self.player = None;
        }
    }

    pub fn play_file_on_top(&mut self) {
        if let Some(player) = &self.player {
            let mut p = player.lock().unwrap();
            p.stop();
            let locked_playlist = self.playlist.lock().expect("fail to get lock on playlist");
            if !locked_playlist.file_list.is_empty() {
                if let Some(n) = locked_playlist.file_list.first() {
                    self.start_play_time = Instant::now(); // before play
                    if let Err(e) = p.start_play(&n.path, Some(self.waittime_between_file_play)) {
                        error!("error in playing file : {}", e);
                    } else {
                        self.play_mod = true;
                    }
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn start_play_time(&self) -> Instant {
        self.start_play_time
    }

    /// get visual notes of the current played file
    pub fn notes(&self) -> Arc<NotesInformations> {
        if let Some(player) = &self.player {
            let p = player.lock().unwrap();
            return p.associated_notes();
        }
        Arc::new(NotesInformations::default())
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
        self.playlist
            .lock()
            .expect("fail to get lock on playlist")
            .skip();

        if self.play_mod {
            self.play_file_on_top();
        }
    }

    pub fn is_playlist_empty(&self) -> bool {
        self.playlist
            .lock()
            .expect("fail to get lock on playlist")
            .file_list
            .is_empty()
    }

    pub fn is_playing(&self) -> bool {
        if let Some(player) = &self.player {
            let p = player.lock().unwrap();
            p.is_playing()
        } else {
            false
        }
    }

    pub fn pause(&self) {
        // send pause
        if let Err(error) = self.commands.send(Command::Pause) {
            debug!("error in sending command :{}", error);
        }
    }
}
