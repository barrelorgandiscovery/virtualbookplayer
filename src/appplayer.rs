//! Hold the Playlist and Player fusion to provide the gui a unique interface

use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
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

    /// Responses the window has not painted yet. `CurrentPlayTime` is collapsed
    /// so a hidden window does not queue one entry per MIDI event.
    pub last_response: Arc<Mutex<Vec<Response>>>,

    /// Playlist advance, shared with the thread that receives player messages.
    /// That thread starts the next file when one ends, so playback does not wait
    /// for the window to be drawn.
    follow: Arc<FollowState>,

    pub start_play_time: Instant,

    /// virtual book
    pub virtual_book: Arc<RwLock<Option<Arc<IndexedVirtualBook>>>>,

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

/// Playlist transport used both by the UI and by the player-response thread.
struct FollowState {
    playlist: Arc<Mutex<PlayList>>,
    play_mod: AtomicBool,
    wait_secs: Mutex<f32>,
    player: Mutex<Option<Arc<Mutex<Box<dyn Player>>>>>,
    /// One transport change at a time: play, stop, skip, or automatic advance.
    transport: Mutex<()>,
}

fn push_player_response(queue: &Mutex<Vec<Response>>, response: Response) {
    let Ok(mut queue) = queue.lock() else {
        return;
    };
    if matches!(response, Response::CurrentPlayTime(_)) {
        if let Some(last) = queue.last_mut() {
            if matches!(last, Response::CurrentPlayTime(_)) {
                *last = response;
                return;
            }
        }
    }
    queue.push(response);
}

impl FollowState {
    fn play_mod(&self) -> bool {
        self.play_mod.load(Ordering::Acquire)
    }

    fn set_play_mod(&self, on: bool) {
        self.play_mod.store(on, Ordering::Release);
    }

    fn set_wait(&self, wait_time: f32) {
        if let Ok(mut wait) = self.wait_secs.lock() {
            *wait = wait_time;
        }
    }

    fn set_player(&self, player: Option<Arc<Mutex<Box<dyn Player>>>>) {
        if let Ok(mut slot) = self.player.lock() {
            *slot = player;
        }
    }

    fn current_player(&self) -> Option<Arc<Mutex<Box<dyn Player>>>> {
        self.player.lock().ok().and_then(|slot| slot.clone())
    }

    /// Enter playlist play mode and start the file at the top, if there is one.
    ///
    /// Play mode stays on when the list is empty, so a file added later can start.
    fn begin(&self) {
        let Ok(_transport) = self.transport.lock() else {
            return;
        };
        self.set_play_mod(true);
        self.play_current_locked();
    }

    /// Start whatever is now at the top of the playlist.
    fn play_current(&self) {
        let Ok(_transport) = self.transport.lock() else {
            return;
        };
        self.play_current_locked();
    }

    fn play_current_locked(&self) {
        let path = {
            let Ok(playlist) = self.playlist.lock() else {
                return;
            };
            playlist
                .file_list
                .first()
                .map(|element| element.path.clone())
        };
        let Some(path) = path else {
            return;
        };
        let Some(player) = self.current_player() else {
            return;
        };
        let wait = self.wait_secs.lock().ok().map(|wait| *wait);
        let Ok(mut player) = player.lock() else {
            return;
        };
        player.stop();
        match player.start_play(&path, wait) {
            Ok(()) => self.play_mod.store(true, Ordering::Release),
            Err(error) => error!("error in playing file : {}", error),
        }
    }

    fn skip_and_maybe_play(&self) {
        let Ok(_transport) = self.transport.lock() else {
            return;
        };
        if let Ok(mut playlist) = self.playlist.lock() {
            playlist.skip();
        }
        if self.play_mod() {
            self.play_current_locked();
        }
    }

    /// Stop the current file and leave playlist play mode.
    fn stop(&self) {
        let Ok(_transport) = self.transport.lock() else {
            return;
        };
        self.set_play_mod(false);
        if let Some(player) = self.current_player() {
            if let Ok(mut player) = player.lock() {
                player.stop();
            }
        }
    }

    /// Continue the playlist after a file ends, when that play is still the current one.
    ///
    /// Runs on the player-response thread, so the next file starts even if the
    /// window is minimized, occluded, or otherwise not drawing frames.
    fn on_file_ended(&self, serial: u64) {
        let Ok(_transport) = self.transport.lock() else {
            return;
        };
        if !self.play_mod() {
            return;
        }
        let current_serial = {
            let Some(player) = self.current_player() else {
                return;
            };
            let Ok(player) = player.lock() else {
                return;
            };
            player.current_play_serial()
        };
        if serial != current_serial {
            return;
        }
        if let Ok(mut playlist) = self.playlist.lock() {
            playlist.skip();
        }
        info!("file ended; continuing the playlist without waiting for the window");
        self.play_current_locked();
    }
}

type PlayerTriplet = (Box<dyn Player>, Receiver<Response>, Sender<Command>);

/// manage asynchrone actions (play, informations retrieve)
impl AppPlayer {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let commands = channel();

        let inner_control_thread = channel::<AppPlayerThreadCommands>();

        let bg_thread_communication = channel::<AppPlayerBackgroundThreadCommands>();

        let playlist = Arc::new(Mutex::new(PlayList::new()));
        let follow = Arc::new(FollowState {
            playlist: Arc::clone(&playlist),
            play_mod: AtomicBool::new(false),
            wait_secs: Mutex::new(0.0),
            player: Mutex::new(None),
            transport: Mutex::new(()),
        });

        let appplayer = Self {
            commands: commands.0,
            player: None,
            playlist,
            last_response: Arc::new(Mutex::new(Vec::new())),
            follow,
            virtual_book: Arc::new(RwLock::new(None)),
            start_play_time: Instant::now() - Duration::from_millis(10_000),
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
                                info!("compute informations for {}", p.path.display());

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
                                        p.additional_informations
                                            .clone_from(&e.additional_informations);
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

    pub fn set_waittime_between_file_play(&self, wait_time: f32) {
        self.follow.set_wait(wait_time);
    }

    pub fn play_mod(&self) -> bool {
        self.follow.play_mod()
    }

    pub fn set_play_mod(&self, on: bool) {
        self.follow.set_play_mod(on);
    }

    /// define the current player, with associated receiver for the player response
    pub fn player(&mut self, player: Option<PlayerTriplet>) {
        // Leave play mode before stopping, so an end-of-file already in flight
        // does not start another tune on the device we are leaving.
        {
            let Ok(_transport) = self.follow.transport.lock() else {
                return;
            };
            self.follow.set_play_mod(false);
            if let Some(old_player_mutex) = &self.player {
                let mut old_player = old_player_mutex.lock().unwrap();
                old_player.stop();
            }
        }

        if let Some((private_player, private_command_sender)) = match player {
            None => None,
            Some(p) => {
                let player_reference = Arc::new(Mutex::new(p.0));
                let last_response = Arc::clone(&self.last_response);
                let follow = Arc::clone(&self.follow);

                let inner_thread_access = self.applayer_sender.clone();
                // create inner control thread of the player
                thread::spawn(move || {
                    #[cfg(feature = "profiling")]
                    profiling::register_thread!("player commands receiver");
                    // println!("start thread for getting responses");
                    while let Ok(response) = p.1.recv() {
                        // println!("received a response from inner player : {:?}", response);

                        match &response {
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
                            Response::CurrentPlayTime(_)
                            | Response::EndOfFile(_)
                            | Response::FileCancelled => {}
                        }

                        // Advance before parking the message for the UI. The window
                        // may not draw again until it is shown, and the playlist
                        // must not wait for that.
                        if let Response::EndOfFile(serial) = &response {
                            follow.on_file_ended(*serial);
                        }
                        push_player_response(&last_response, response);
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
            self.follow.set_player(Some(Arc::clone(&private_player)));
            self.player = Some(private_player);
            self.commands = private_command_sender;
        } else {
            self.follow.set_player(None);
            self.player = None;
        }
    }

    pub fn play_file_on_top(&self) {
        self.follow.play_current();
    }

    /// Turn playlist play on and start the current file, if the list is not empty.
    pub fn begin_playlist(&self) {
        self.follow.begin();
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

    /// stop the play and leave playlist play mode
    pub fn stop(&self) {
        self.follow.stop();
    }

    /// next file, and "unpop" the currently played
    pub fn next(&self) {
        self.follow.skip_and_maybe_play();
    }

    pub fn is_playlist_empty(&self) -> bool {
        self.playlist
            .lock()
            .expect("fail to get lock on playlist")
            .file_list
            .is_empty()
    }

    /// Get the name of the next file in the playlist (if any)
    /// Returns None if there's no next file or if the playlist is locked
    pub fn next_file_name(&self) -> Option<String> {
        if let Ok(playlist) = self.playlist.lock() {
            if playlist.file_list.len() > 1 {
                Some(playlist.file_list[1].name.clone())
            } else {
                None
            }
        } else {
            None
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

    /// Freeze or continue the current file. Playlist play mode is left unchanged.
    pub fn set_tune_paused(&self, paused: bool) {
        if let Err(error) = self.commands.send(Command::Pause(paused)) {
            debug!("error in sending pause command :{}", error);
        }
    }

    pub fn is_paused(&self) -> bool {
        if let Some(player) = &self.player {
            let player = player.lock().unwrap();
            player.is_paused()
        } else {
            false
        }
    }
}

#[cfg(test)]
mod follow_tests {
    use super::*;
    use player::{FileInformationsConstructor, NotesInformations};
    use std::error::Error;
    use std::path::PathBuf;

    struct StubPlayer {
        serial: u64,
        started: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl Player for StubPlayer {
        fn start_play(
            &mut self,
            filename: &PathBuf,
            _start_time: Option<f32>,
        ) -> Result<(), Box<dyn Error>> {
            self.serial = self.serial.wrapping_add(1);
            self.started.lock().unwrap().push(filename.clone());
            Ok(())
        }

        fn stop(&mut self) {}

        fn is_paused(&self) -> bool {
            false
        }

        fn is_playing(&self) -> bool {
            false
        }

        fn current_play_time(&self) -> i64 {
            0
        }

        fn associated_notes(&self) -> Arc<NotesInformations> {
            Arc::new(NotesInformations::default())
        }

        fn create_information_getter(
            &self,
        ) -> Result<Box<dyn FileInformationsConstructor>, Box<dyn Error>> {
            Err("unused".into())
        }

        fn current_play_serial(&self) -> u64 {
            self.serial
        }
    }

    fn follow_with(files: &[&str], serial: u64) -> (Arc<FollowState>, Arc<Mutex<Vec<PathBuf>>>) {
        let started = Arc::new(Mutex::new(Vec::new()));
        let playlist = Arc::new(Mutex::new(PlayList::new()));
        {
            let mut playlist = playlist.lock().unwrap();
            for file in files {
                playlist.add_from_path(&PathBuf::from(file));
            }
        }
        let player: Arc<Mutex<Box<dyn Player>>> = Arc::new(Mutex::new(Box::new(StubPlayer {
            serial,
            started: Arc::clone(&started),
        })));
        let follow = Arc::new(FollowState {
            playlist,
            play_mod: AtomicBool::new(true),
            wait_secs: Mutex::new(0.0),
            player: Mutex::new(Some(player)),
            transport: Mutex::new(()),
        });
        (follow, started)
    }

    #[test]
    fn end_of_file_starts_the_next_tune_without_a_frame() {
        let (follow, started) = follow_with(&["/tunes/a.mid", "/tunes/b.mid"], 1);

        follow.on_file_ended(1);

        let names: Vec<_> = follow
            .playlist
            .lock()
            .unwrap()
            .file_list
            .iter()
            .map(|element| element.name.clone())
            .collect();
        assert_eq!(names, vec!["b.mid".to_string()]);
        assert_eq!(
            started.lock().unwrap().clone(),
            vec![PathBuf::from("/tunes/b.mid")]
        );
    }

    #[test]
    fn a_stale_end_does_not_skip_the_file_already_started() {
        let (follow, started) = follow_with(&["/tunes/a.mid", "/tunes/b.mid"], 1);

        follow.on_file_ended(1);
        follow.on_file_ended(1);

        assert_eq!(follow.playlist.lock().unwrap().file_list.len(), 1);
        assert_eq!(started.lock().unwrap().len(), 1);
    }

    #[test]
    fn end_of_file_does_nothing_when_playlist_play_is_off() {
        let (follow, started) = follow_with(&["/tunes/a.mid", "/tunes/b.mid"], 1);
        follow.set_play_mod(false);

        follow.on_file_ended(1);

        assert_eq!(follow.playlist.lock().unwrap().file_list.len(), 2);
        assert!(started.lock().unwrap().is_empty());
    }

    #[test]
    fn play_times_collapse_to_the_latest_pending_sample() {
        let queue = Mutex::new(Vec::new());
        push_player_response(&queue, Response::CurrentPlayTime(Duration::from_millis(10)));
        push_player_response(&queue, Response::CurrentPlayTime(Duration::from_millis(20)));
        push_player_response(&queue, Response::FileCancelled);
        push_player_response(&queue, Response::CurrentPlayTime(Duration::from_millis(30)));

        let queue = queue.lock().unwrap();
        assert!(
            matches!(queue[0], Response::CurrentPlayTime(time) if time == Duration::from_millis(20))
        );
        assert!(matches!(queue[1], Response::FileCancelled));
        assert!(
            matches!(queue[2], Response::CurrentPlayTime(time) if time == Duration::from_millis(30))
        );
    }
}
