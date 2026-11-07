use std::{
    collections::HashMap,
    error::Error,
    fs::File,
    io::BufReader,
    path::PathBuf,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex, MutexGuard,
    },
    thread::{self},
    time::{Duration, Instant},
};

use bookparsing::VirtualBook;
use midir::{MidiOutput, MidiOutputConnection};
use nodi::{
    midly::{Format, MidiMessage, Smf},
    timers::Ticker,
    Event, MidiEvent, Sheet, Timer,
};

use crate::{
    Command, FileInformations, FileInformationsConstructor, NotesDisplayInformations,
    NotesInformations, PlainNoteWithChannel, Player, PlayerFactory, Response,
};

use std::convert::TryFrom;

use log::{debug, error, info, warn};

use thread_priority::*;

use self::midiconverter::{convert, create_conversion_from_scale, read_conversion, Conversion};

mod midiconverter;

// 120 bpm default tempo for files that does not have tempo signature in it
// 48 ticks per quarter note
// 4/4 signature

// microseconds per beat
const BEAT_TIME_IN_MICROSECOND: u32 = 60 * 1_000_000 / 120;
const DEFAULT_TEMPO_IF_NOT_SET_IN_FILE: u32 = BEAT_TIME_IN_MICROSECOND;

/// Midi device player factory
pub struct MidiPlayerFactory {
    pub device_no: usize,
}

#[profiling::all_functions]
impl PlayerFactory for MidiPlayerFactory {
    fn create(
        &self,
        sender: Sender<Response>,
        receiver: Receiver<Command>,
    ) -> Result<Box<dyn Player>, Box<dyn Error>> {
        println!("List connections");

        MidiPlayerFactory::list_devices().expect("Error in listing midi devices");

        let midi_out = MidiPlayerFactory::get_connection(self.device_no)?;

        println!("Opening connection");

        let cancels = channel();

        Ok(Box::new(MidiPlayer {
            midi_output_connection: Arc::new(Mutex::new(midi_out)),
            output: Arc::new(Mutex::new(sender)),
            cancel: cancels.0,
            commands: Arc::new(Mutex::new(receiver)),
            ispaused: Arc::new(Mutex::new(false)),
            isplaying: Arc::new(Mutex::new(false)),
            notes: Arc::new(Mutex::new(Arc::new(NotesInformations::default()))),
            play_serial: 0,
        }))
    }

    fn create_information_getter(
        &self,
    ) -> Result<Box<dyn FileInformationsConstructor>, Box<dyn Error>> {
        Ok(Box::new(MidiFileInformationsConstructor {}))
    }
}

/// midi device reference
pub struct DeviceInformation {
    pub no: usize,
    pub label: String,
}

#[profiling::all_functions]
impl MidiPlayerFactory {
    pub fn get_connection(n: usize) -> Result<MidiOutputConnection, Box<dyn Error>> {
        let midi_out = MidiOutput::new("play_midi")?;

        let out_ports = midi_out.ports();
        if out_ports.is_empty() {
            return Err("no MIDI output device detected".into());
        }
        if n >= out_ports.len() {
            return Err(format!("only {} MIDI devices detected", out_ports.len()).into());
        }

        let out_port = &out_ports[n];
        let out = midi_out.connect(out_port, "cello-tabs")?;
        Ok(out)
    }

    pub fn list_all_devices() -> Result<Vec<DeviceInformation>, Box<dyn Error>> {
        let midi_out = MidiOutput::new("play_midi")?;

        let out_ports = midi_out.ports();

        let mut returned: Vec<DeviceInformation> = vec![];

        if out_ports.is_empty() {
            println!("No active MIDI output device detected.");
        } else {
            for (i, p) in out_ports.iter().enumerate() {
                let label = format!(
                    "#{}: {}",
                    i,
                    midi_out
                        .port_name(p)
                        .as_deref()
                        .unwrap_or("<no device name>")
                );
                returned.push(DeviceInformation { no: i, label });
            }
        }
        Ok(returned)
    }

    /// list devices on the stdout
    pub fn list_devices() -> Result<(), Box<dyn Error>> {
        let devices = MidiPlayerFactory::list_all_devices()?;
        devices.iter().for_each(|d| {
            println!("{}", &d.label);
        });
        Ok(())
    }
}

pub struct MidiFileInformationsConstructor {}

/// file information trait, specific to midi, and midi controlled equipments
impl FileInformationsConstructor for MidiFileInformationsConstructor {
    fn compute(&mut self, filename: &PathBuf) -> Result<FileInformations, Box<dyn Error>> {
        match read_all_kind_of_files(filename, None) {
            Ok(res) => {
                let result = res
                    .0
                    .notes
                    .iter()
                    .fold(Duration::new(0, 0), |acc, n| acc.max(n.start + n.length));

                // result
                Ok(FileInformations {
                    duration: Some(result),
                })
            }
            Err(e) => Err(e),
        }
    }
}

/// midi player
pub struct MidiPlayer {
    /// midi connection
    midi_output_connection: Arc<Mutex<MidiOutputConnection>>,

    /// channel to send message to outbox application
    /// send informations / response to owner
    output: Arc<Mutex<Sender<Response>>>,

    /// cancel channel
    cancel: Sender<bool>,

    // commands
    commands: Arc<Mutex<Receiver<Command>>>,

    ispaused: Arc<Mutex<bool>>,

    /// is playing, is the engine is playing a file, this return true
    isplaying: Arc<Mutex<bool>>,

    /// note representation for the display
    // shared between threads
    notes: Arc<Mutex<Arc<NotesInformations>>>,

    /// Bumped on every `start_play`. Sent back with `EndOfFile`.
    play_serial: u64,
}

impl Drop for MidiPlayer {
    fn drop(&mut self) {
        // drop(*self.midi_output_connection);
        // drop(*self.output);
    }
}

#[allow(dead_code)]
fn send_panic(con: &mut MutexGuard<MidiOutputConnection>) {
    // panic
    let mut buf = Vec::new();
    buf.push(0xcc);
    buf.push(123);
    if let Err(e) = con.send(&buf) {
        error!("error in sending panic code :{}", e);
    }
}

/// CC 120 (All Sound Off) and CC 123 (All Notes Off) on every channel.
fn all_sound_off_messages() -> [[u8; 3]; 32] {
    let mut messages = [[0u8; 3]; 32];
    for ch in 0..16u8 {
        let status = 0xB0 | ch;
        messages[ch as usize * 2] = [status, 120, 0];
        messages[ch as usize * 2 + 1] = [status, 123, 0];
    }
    messages
}

fn all_sound_off(con: &mut MutexGuard<MidiOutputConnection>) {
    for message in all_sound_off_messages() {
        if let Err(e) = con.send(&message) {
            warn!("fail to send all-sound-off {message:?}: {e}");
        }
    }
}

fn note_on_message(channel: u8, key: u8, vel: u8) -> [u8; 3] {
    [0x90 | (channel & 0x0F), key & 0x7F, vel & 0x7F]
}

/// Notes already sent and not yet released: `(channel, key) -> velocity`.
fn track_sounding(sounding: &mut HashMap<(u8, u8), u8>, channel: u8, message: &MidiMessage) {
    match message {
        MidiMessage::NoteOff { key, .. } => {
            sounding.remove(&(channel, key.as_int()));
        }
        MidiMessage::NoteOn { key, vel } => {
            let key = key.as_int();
            let vel = vel.as_int();
            if vel == 0 {
                sounding.remove(&(channel, key));
            } else {
                sounding.insert((channel, key), vel);
            }
        }
        _ => {}
    }
}

fn replay_sounding(con: &mut MutexGuard<MidiOutputConnection>, sounding: &HashMap<(u8, u8), u8>) {
    for (&(channel, key), &vel) in sounding {
        let message = note_on_message(channel, key, vel);
        if let Err(e) = con.send(&message) {
            warn!("fail to resume note {message:?}: {e}");
        }
    }
}

fn drain_commands(commands: &Mutex<Receiver<Command>>) {
    if let Ok(receiver) = commands.lock() {
        while receiver.try_recv().is_ok() {}
    }
}

/// Apply one explicit pause or resume. A repeated value is ignored.
fn poll_pause_command(
    commands: &Mutex<Receiver<Command>>,
    ispaused: &Mutex<bool>,
    con: &mut MutexGuard<MidiOutputConnection>,
    sounding: &HashMap<(u8, u8), u8>,
    output: &Mutex<Sender<Response>>,
    play_time: Duration,
) {
    let command = {
        let Ok(receiver) = commands.lock() else {
            return;
        };
        receiver.try_recv().ok()
    };
    let Some(command) = command else {
        return;
    };
    let Command::Pause(pause) = command else {
        debug!("command not yet supported");
        return;
    };

    let currently = ispaused.lock().map(|paused| *paused).unwrap_or(false);
    if pause == currently {
        return;
    }
    if let Ok(mut paused) = ispaused.lock() {
        *paused = pause;
    }
    if pause {
        all_sound_off(con);
    } else {
        replay_sounding(con, sounding);
    }
    if let Ok(output_locked) = output.lock() {
        let _ = output_locked.send(Response::CurrentPlayTime(play_time));
    }
}

/// Sleep `total`, but keep polling pause and cancel.
/// Pause freezes the remaining sleep. Returns true when the file was cancelled.
fn sleep_interruptible(
    total: Duration,
    cancel: &Receiver<bool>,
    commands: &Mutex<Receiver<Command>>,
    ispaused: &Mutex<bool>,
    con: &mut MutexGuard<MidiOutputConnection>,
    sounding: &HashMap<(u8, u8), u8>,
    output: &Mutex<Sender<Response>>,
    play_time: Duration,
) -> bool {
    let slice = Duration::from_millis(50);
    let mut left = total;
    loop {
        if cancel.try_recv().is_ok() {
            return true;
        }
        poll_pause_command(commands, ispaused, con, sounding, output, play_time);
        let paused = ispaused.lock().map(|flag| *flag).unwrap_or(false);
        if paused {
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        if left.is_zero() {
            return false;
        }
        let step = if left > slice { slice } else { left };
        thread::sleep(step);
        left = left.saturating_sub(step);
    }
}

fn finish_cancelled(
    con: &mut MutexGuard<MidiOutputConnection>,
    isplaying: &Mutex<bool>,
    ispaused: &Mutex<bool>,
    output: &Mutex<Sender<Response>>,
) {
    all_notes_off(con);
    if let Ok(mut paused) = ispaused.lock() {
        *paused = false;
    }
    if let Ok(mut playing) = isplaying.lock() {
        *playing = false;
    }
    if let Ok(output_locked) = output.lock() {
        output_locked.send(Response::FileCancelled).unwrap();
    }
}

// sending all note off
fn all_notes_off(con: &mut MutexGuard<MidiOutputConnection>) {
    let mut buf = Vec::new();
    for ch in 0..16 {
        for note in 0..=127 {
            let msg = MidiEvent {
                channel: ch.into(),
                message: MidiMessage::NoteOff {
                    key: note.into(),
                    vel: 127.into(),
                },
            };

            buf.clear();
            let _ = msg.write(&mut buf); // ignore return
            if let Err(e) = con.send(&buf) {
                warn!("fail to send stop note : {:?} {:?}", msg, e);
            }
        }
    }
}

#[profiling::function]
fn read_midi_file(
    filename: &PathBuf,
    start_wait: Option<f32>,
) -> Result<(Arc<NotesInformations>, Ticker, Sheet), Box<dyn Error>> {
    // Load bytes first
    let file_content_data = std::fs::read(filename)?;

    // parse it
    let smf = Smf::parse(&file_content_data)?;

    // get note display
    let notes = Arc::new(to_notes(&smf, &start_wait)?);

    // deconstruct the elements
    let Smf { header, tracks } = smf;

    let mut timer = Ticker::try_from(header.timing)?;
    debug!("timer : {:?}", &timer);
    timer.change_tempo(DEFAULT_TEMPO_IF_NOT_SET_IN_FILE);

    let sheet = match header.format {
        Format::SingleTrack | Format::Sequential => Sheet::sequential(&tracks),
        Format::Parallel => Sheet::parallel(&tracks),
    };

    let mut notes_informations = NotesInformations::default();
    notes_informations.notes = notes;

    Ok((Arc::new(notes_informations), timer, sheet))
}

#[profiling::function]
fn resolve_conversion(vb: &VirtualBook) -> Result<Option<Conversion>, Box<dyn Error>> {
    let scale_name = vb.scale.name.clone();
    let conversion_file = scale_name + ".yml";

    if std::fs::metadata(&conversion_file).is_err() {
        info!("create the conversion from scale definition");
        return Ok(Some(create_conversion_from_scale(&vb.scale.definition)?));
    }

    let result_open = File::open(PathBuf::from(&conversion_file));
    if let Err(e) = result_open {
        return Err(format!("error opening mapping :{}, : {:?}", &conversion_file, &e).into());
    }
    let f_scale = result_open?;

    let mut reader = BufReader::new(f_scale);
    let conversion = read_conversion(&mut reader)?;
    Ok(Some(conversion))
}

#[profiling::function]
fn read_book_file(
    filename: &PathBuf, // must be a book
    // extension : external_dir_for_overload: &PathBuf,
    start_wait: Option<f32>,
) -> Result<(Arc<NotesInformations>, Ticker, Sheet), Box<dyn Error>> {
    // book parsing
    let file = File::open(filename)?;
    let mut reader = BufReader::new(&file);
    let vb = bookparsing::read_book_stream(&mut reader)?;
    let found_conversion = resolve_conversion(&vb)?;

    let seconds_duration = match start_wait {
        Some(secs) => Duration::from_secs_f32(secs),
        None => Duration::ZERO,
    };

    return match found_conversion {
        None => Err(format!("no conversion found for {}", &filename.to_string_lossy()).into()),
        Some(conversion) => {
            let resultsmf = convert(&vb, &conversion)?;

            let plain_notes: Arc<Vec<PlainNoteWithChannel>> = Arc::new(
                vb.holes
                    .holes
                    .iter()
                    .filter(|hole| hole.timestamp >= 0 && hole.length >= 0)
                    .map(|hole| {
                        assert!(hole.length >= 0);
                        assert!(hole.timestamp >= 0);

                        PlainNoteWithChannel {
                            channel: 0,
                            start: Duration::from_micros(hole.timestamp as u64) + seconds_duration, // todo check this fact
                            length: Duration::from_micros(hole.length as u64),
                            note: hole.track as u8,
                            track: hole.track,
                        }
                    })
                    .collect(),
            );

            // deconstruct the elements
            let Smf { header, tracks } = resultsmf;

            let mut timer = Ticker::try_from(header.timing)?;
            debug!("timer : {:?}", &timer);
            timer.change_tempo(DEFAULT_TEMPO_IF_NOT_SET_IN_FILE);

            let sheet = match header.format {
                Format::SingleTrack | Format::Sequential => Sheet::sequential(&tracks),
                Format::Parallel => Sheet::parallel(&tracks),
            };

            let mut notes_informations = NotesInformations::default();

            notes_informations.notes = plain_notes;

            notes_informations.display_informations = NotesDisplayInformations {
                first_axis: vb.scale.definition.firsttrackdistance,
                inter_axis: vb.scale.definition.intertrackdistance,
                track_width: vb.scale.definition.defaulttrackheight,
                width: vb.scale.definition.width,
                preferred_view_inversed: vb.scale.definition.ispreferredviewinverted,
            };

            Ok((Arc::new(notes_informations), timer, sheet))
        }
    };
}

#[profiling::function]
fn read_all_kind_of_files(
    filename: &PathBuf, // must be a book
    // extension : external_dir_for_overload: &PathBuf,
    start_wait: Option<f32>,
) -> Result<(Arc<NotesInformations>, Ticker, Sheet), Box<dyn Error>> {
    info!("reading {:?}", filename);
    let ext_option = filename.extension();
    if let Some(ext) = ext_option {
        match ext.to_ascii_lowercase().to_string_lossy().as_ref() {
            "mid" => {
                info!("reading midi file : {:?}", filename);
                return read_midi_file(filename, start_wait);
            }
            "book" => {
                info!("reading book file : {:?}", filename);
                return read_book_file(filename, start_wait);
            }
            _ => {
                warn!("this file type : {:?} is not known", filename);
            }
        }
    }
    Err(format!("no extension given for the file {:?}", &filename).into())
}

/// Player trait implementation
#[profiling::all_functions]
impl Player for MidiPlayer {
    fn start_play(
        &mut self,
        filename: &PathBuf,
        start_wait: Option<f32>,
    ) -> Result<(), Box<dyn Error>> {
        #[cfg(feature = "profiling")]
        profiling::scope!("Prepare start play");

        {
            self.output
                .lock()
                .unwrap()
                .send(Response::CurrentPlayTime(Duration::from_secs(0)))
                .unwrap();
        }

        let _ = self.cancel.send(true); // don't handle the error

        // silence the output
        self.silence();

        let (sender, receiver) = channel();
        self.cancel = sender;

        let con = Arc::clone(&self.midi_output_connection);

        let isplaying_info = Arc::clone(&self.isplaying);

        let output_reference = Arc::clone(&self.output);

        let filename_closure = filename.clone();
        let start_wait_closure = start_wait;

        let notes_access = Arc::clone(&self.notes);

        let commands = Arc::clone(&self.commands);

        self.play_serial = self.play_serial.wrapping_add(1);
        let play_serial = self.play_serial;

        let ispaused = Arc::new(Mutex::new(false));
        self.ispaused = ispaused.clone();

        // thread spawned interpret the Midi event and send them on the line
        thread::spawn(move || {
            profiling::register_thread!("player thread");
            // Try to set max priority - this may fail on Linux without root/CAP_SYS_NICE (OS error 13)
            // This is expected and not critical - playback will work fine with normal priority
            if let Err(e) = set_current_thread_priority(ThreadPriority::Max) {
                debug!("Could not set max priority to player thread (this is normal without root privileges): {:?}", e);
            }

            let mut buf = Vec::new();
            let mut total_duration = Duration::new(0, 0);
            let mut ticks_counter = 0_u32;

            if let Ok(mut con) = con.lock() {
                debug!("midi connexion aquired");

                // A pause clicked as the previous file ended must not freeze this one.
                drain_commands(&commands);
                all_notes_off(&mut con);

                if let Ok(output_locked) = output_reference.lock() {
                    output_locked
                        .send(Response::CurrentPlayTime(Duration::ZERO))
                        .unwrap();
                }

                let start_time = Instant::now();
                // load the file
                let read_result = read_all_kind_of_files(&filename_closure, start_wait_closure);

                match read_result {
                    Err(e) => {
                        error!("error in reading file : {:?}", e);
                        // Still report the end, so the playlist can move on
                        // without waiting for a window redraw.
                        if let Ok(output_locked) = output_reference.lock() {
                            let _ = output_locked.send(Response::EndOfFile(play_serial));
                        }
                    }

                    Ok((notes_informations, mut timer, midi_sheet)) => {
                        info!(
                            "File read and converted in {} ms",
                            (Instant::now() - start_time).as_millis()
                        );

                        if let Ok(mut m) = isplaying_info.lock() {
                            *m = true;
                        }
                        {
                            // change the note informations
                            let mut note_guard = notes_access.try_lock().unwrap();
                            *note_guard = Arc::clone(&notes_informations);
                        }

                        let wait_time = if let Some(w) = start_wait {
                            Duration::from_secs_f32(w)
                        } else {
                            Duration::ZERO
                        };

                        // send message FilePlayStarted
                        if let Ok(output_locked) = output_reference.lock() {
                            let filename = filename_closure.clone();
                            if let Err(err_send_file_started) =
                                output_locked.send(Response::FilePlayStarted((
                                    String::from(filename.to_string_lossy()),
                                    notes_informations,
                                )))
                            {
                                error!(
                                    "error sending start of playing : {:?}",
                                    err_send_file_started
                                );
                            }
                        }

                        // Holes already sounding. Pause silences the organ; resume
                        // replays these NoteOns so held notes come back.
                        let mut sounding: HashMap<(u8, u8), u8> = HashMap::new();

                        // start waiting, before the play
                        let _start_wait_time = Instant::now();
                        if let Some(wait) = start_wait {
                            let mut remain = wait;
                            const INCREMENT: f32 = 0.2;
                            while remain > 0.0f32 {
                                if receiver.try_recv().is_ok() {
                                    finish_cancelled(
                                        &mut con,
                                        &isplaying_info,
                                        &ispaused,
                                        &output_reference,
                                    );
                                    return;
                                }

                                let played = Duration::from_secs_f32((wait - remain).max(0.0));
                                poll_pause_command(
                                    &commands,
                                    &ispaused,
                                    &mut con,
                                    &sounding,
                                    &output_reference,
                                    played,
                                );
                                let paused = ispaused.lock().map(|flag| *flag).unwrap_or(false);
                                if paused {
                                    // Freeze the remaining lead-in. Do not cancel the file.
                                    thread::sleep(Duration::from_millis(100));
                                    continue;
                                }

                                thread::sleep(Duration::from_secs_f32(INCREMENT));
                                remain -= INCREMENT;

                                if let Ok(output_locked) = output_reference.lock() {
                                    output_locked
                                        .send(Response::CurrentPlayTime(Duration::from_secs_f32(
                                            (wait - remain).max(0.0),
                                        )))
                                        .unwrap();
                                }
                            }
                        }

                        let mut iter_moment = midi_sheet.iter();

                        loop {
                            // for moment in midi_sheet {
                            if receiver.try_recv().is_ok() {
                                finish_cancelled(
                                    &mut con,
                                    &isplaying_info,
                                    &ispaused,
                                    &output_reference,
                                );
                                return;
                            }

                            poll_pause_command(
                                &commands,
                                &ispaused,
                                &mut con,
                                &sounding,
                                &output_reference,
                                total_duration + wait_time,
                            );

                            if ispaused.lock().map(|flag| *flag).unwrap_or(false) {
                                // Hold the playhead. One CurrentPlayTime was sent
                                // on the transition; do not walk the clock here.
                                thread::sleep(Duration::from_millis(100));
                                continue;
                            }

                            let current = iter_moment.next();
                            if let Some(moment) = current {
                                if !moment.is_empty() {
                                    if let Ok(mut m) = isplaying_info.lock() {
                                        *m = true;
                                    }
                                    let d = timer.sleep_duration(ticks_counter);

                                    if sleep_interruptible(
                                        d,
                                        &receiver,
                                        &commands,
                                        &ispaused,
                                        &mut con,
                                        &sounding,
                                        &output_reference,
                                        total_duration + wait_time,
                                    ) {
                                        finish_cancelled(
                                            &mut con,
                                            &isplaying_info,
                                            &ispaused,
                                            &output_reference,
                                        );
                                        return;
                                    }

                                    total_duration += d;
                                    // total_duration = Instant::now() - start_play_time;

                                    ticks_counter = 0;

                                    #[cfg(feature = "profiling")]
                                    profiling::scope!("play moment events");

                                    // let current_time = Instant::now() - _start_wait_time;

                                    for event in &moment.events {
                                        match event {
                                            Event::Tempo(val) => {
                                                timer.change_tempo(*val);
                                            }

                                            Event::Midi(msg) => {
                                                track_sounding(
                                                    &mut sounding,
                                                    msg.channel.as_int(),
                                                    &msg.message,
                                                );
                                                buf.clear();
                                                let _ = msg.write(&mut buf);
                                                let _ = con.send(&buf);
                                            }
                                            _ => (),
                                        };
                                    }

                                    if let Ok(output_locked) = output_reference.lock() {
                                        output_locked
                                            .send(Response::CurrentPlayTime(
                                                total_duration + wait_time,
                                            ))
                                            .unwrap();
                                    }
                                }

                                ticks_counter += 1;
                            } else {
                                info!("end of moments");
                                break;
                            }
                        }

                        if let Ok(mut m) = isplaying_info.lock() {
                            *m = false;
                        }

                        if let Ok(output_locked) = output_reference.lock() {
                            if let Err(err_send_end_of_file) =
                                output_locked.send(Response::EndOfFile(play_serial))
                            {
                                error!("error sending end of file : {:?}", err_send_end_of_file);
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    // is in pause ?
    fn is_paused(&self) -> bool {
        let playing = self.isplaying.lock().map(|flag| *flag).unwrap_or(false);
        if !playing {
            return false;
        }
        self.ispaused.lock().map(|paused| *paused).unwrap_or(false)
    }

    fn stop(&mut self) {
        // Try to send cancel signal - if channel is closed, thread already finished (this is normal)
        if let Err(e) = self.cancel.send(true) {
            debug!(
                "Could not send cancel order (thread may have already finished): {}",
                e
            );
        }
    }

    fn is_playing(&self) -> bool {
        if let Ok(m) = self.isplaying.lock() {
            *m
        } else {
            false
        }
    }

    fn current_play_time(&self) -> i64 {
        todo!()
    }

    fn associated_notes(&self) -> Arc<NotesInformations> {
        Arc::clone(&self.notes.lock().unwrap())
    }

    fn create_information_getter(
        &self,
    ) -> Result<Box<dyn FileInformationsConstructor>, Box<dyn Error>> {
        Ok(Box::new(MidiFileInformationsConstructor {}))
    }

    fn current_play_serial(&self) -> u64 {
        self.play_serial
    }
}

#[profiling::all_functions]
/// midi player object/structure
impl MidiPlayer {
    /// create a new midi player structure, given the midi output and command/information send channel
    pub fn new(
        con: MidiOutputConnection,
        output: Sender<Response>,
        command: Receiver<Command>,
    ) -> Self {
        let con = Arc::new(Mutex::new(con));

        let c = channel();

        Self {
            output: Arc::new(Mutex::new(output)),
            cancel: c.0,
            commands: Arc::new(Mutex::new(command)),
            midi_output_connection: con,
            isplaying: Arc::new(Mutex::new(false)),
            ispaused: Arc::new(Mutex::new(false)),
            notes: Arc::new(Mutex::new(Arc::new(NotesInformations::default()))),
            play_serial: 0,
        }
    }

    /// silence all notes (sending cmd stop all)
    fn silence(&self) {
        let mut con = self.midi_output_connection.lock().unwrap();
        let _ = con.send(&[0xb0, 123]);
        let _ = con.send(&[0xb0, 120]);
    }
}

/// convert midi file to notes
pub fn to_notes(
    smf: &Smf,
    start_wait: &Option<f32>,
) -> Result<Vec<PlainNoteWithChannel>, Box<dyn Error>> {
    let Smf { header, tracks } = smf;
    let miditiming = header.timing;
    let mut timer = Ticker::try_from(miditiming)?;

    debug!("timer : {:?}", &timer);
    timer.change_tempo(DEFAULT_TEMPO_IF_NOT_SET_IN_FILE);

    let sheet = match header.format {
        Format::SingleTrack | Format::Sequential => Sheet::sequential(tracks),
        Format::Parallel => Sheet::parallel(tracks),
    };

    let shift_duration = if start_wait.is_some() {
        Duration::from_secs_f32(start_wait.unwrap())
    } else {
        Duration::ZERO
    };

    // note activation
    let mut note_state: Vec<Vec<Duration>> = vec![vec![]; 16 * 128];
    let mut notes: Vec<PlainNoteWithChannel> = vec![];

    let mut counter = 0_u32;
    let mut total_duration = Duration::new(0, 0);

    for moment in sheet.iter() {
        if !moment.is_empty() {
            let d = timer.sleep_duration(counter);
            total_duration += d;
            counter = 0;
            for event in &moment.events {
                match event {
                    Event::Tempo(val) => timer.change_tempo(*val),

                    Event::Midi(msg) => match msg.message {
                        MidiMessage::NoteOff { key, vel: _ } => {
                            let uchannel: u16 = msg.channel.as_int().into();
                            let key = key.as_int();
                            let index: usize = (uchannel as usize) * 128 + key as usize;
                            let channel_note_vec = &mut note_state[index];
                            if !channel_note_vec.is_empty() {
                                match channel_note_vec.pop() {
                                    Some(d) => notes.push(PlainNoteWithChannel {
                                        channel: uchannel,
                                        note: key,
                                        track: key as u16,
                                        start: d + shift_duration,
                                        length: total_duration - d,
                                    }),
                                    None => {}
                                }
                            }
                        }
                        MidiMessage::NoteOn { key, vel } => {
                            let uchannel: u16 = msg.channel.as_int().into();
                            let key = key.as_int();
                            let index: usize = (uchannel as usize) * 128 + key as usize;

                            if vel == 0 {
                                // note off
                                let channel_note_vec = &mut note_state[index];
                                if !channel_note_vec.is_empty() {
                                    match channel_note_vec.pop() {
                                        Some(d) => notes.push(PlainNoteWithChannel {
                                            channel: uchannel,
                                            note: key,
                                            track: key as u16,
                                            start: d + shift_duration,
                                            length: total_duration - d,
                                        }),
                                        None => {}
                                    }
                                }
                            } else {
                                let channel_note_vec = &mut note_state[index];

                                channel_note_vec.push(total_duration);
                            }
                        }

                        _ => {}
                    },
                    _ => (),
                };
                // println!("{:?} {:?}", &total_duration, &event);
            }
        }
        counter += 1;
    }
    Ok(notes)
}

#[cfg(test)]
mod tune_pause_tests {
    use super::{all_sound_off_messages, note_on_message, track_sounding};
    use nodi::midly::{num::u7, MidiMessage};
    use std::collections::HashMap;

    fn note_on(key: u8, vel: u8) -> MidiMessage {
        MidiMessage::NoteOn {
            key: u7::new(key),
            vel: u7::new(vel),
        }
    }

    #[test]
    fn all_sound_off_is_three_byte_cc_on_every_channel() {
        let messages = all_sound_off_messages();
        assert_eq!(messages.len(), 32);
        assert_eq!(messages[0], [0xB0, 120, 0]);
        assert_eq!(messages[1], [0xB0, 123, 0]);
        assert_eq!(messages[30], [0xBF, 120, 0]);
        assert_eq!(messages[31], [0xBF, 123, 0]);
    }

    #[test]
    fn resume_keeps_held_notes_and_drops_released_ones() {
        let mut sounding = HashMap::new();
        track_sounding(&mut sounding, 2, &note_on(60, 100));
        track_sounding(&mut sounding, 2, &note_on(64, 80));
        track_sounding(
            &mut sounding,
            2,
            &MidiMessage::NoteOff {
                key: u7::new(60),
                vel: u7::new(0),
            },
        );
        track_sounding(&mut sounding, 2, &note_on(64, 0));

        assert!(sounding.is_empty());

        track_sounding(&mut sounding, 0, &note_on(67, 90));
        assert_eq!(note_on_message(0, 67, 90), [0x90, 67, 90]);
        assert_eq!(sounding.get(&(0, 67)), Some(&90));
    }
}
