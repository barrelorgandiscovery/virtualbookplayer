use std::{
    error::Error,
    path::PathBuf,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex, MutexGuard,
    },
    thread,
    time::Duration,
};

use midir::{MidiOutput, MidiOutputConnection};
use nodi::{
    midly::{Format, MidiMessage, Smf},
    timers::Ticker,
    Event, MidiEvent, Sheet, Timer,
};

use crate::{
    Command, FileInformations, FileInformationsConstructor, Note, Player, PlayerFactory, Response,
};

use std::convert::TryFrom;

use log::{debug, error, warn};

use thread_priority::*;

mod midifactory;

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

impl PlayerFactory for MidiPlayerFactory {
    fn create(
        &self,
        sender: Sender<Response>,
        _receiver: Receiver<Command>,
    ) -> Result<Box<dyn Player>, Box<dyn Error>> {
        println!("List connections");

        MidiPlayerFactory::list_devices().expect("Error in listing midi devices");

        let midi_out = MidiPlayerFactory::get_connection(self.device_no)?;

        println!("\nOpening connection");

        let cancels = channel();

        Ok(Box::new(MidiPlayer {
            midi_output_connection: Arc::new(Mutex::new(midi_out)),
            output: Arc::new(Mutex::new(sender)),
            cancel: cancels.0,
            isplaying: Arc::new(Mutex::new(false)),
            notes: Arc::new(Mutex::new(Arc::new(vec![]))),
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
                returned.push(DeviceInformation {
                    no: i,
                    label: label,
                });
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

impl FileInformationsConstructor for MidiFileInformationsConstructor {
    fn compute(&mut self, filename: &PathBuf) -> Result<Arc<FileInformations>, Box<dyn Error>> {
        // Load bytes first
        let file_content_data = std::fs::read(filename)?;
        // parse it
        let smf = Smf::parse(&file_content_data)?;
        // get note display
        let notes = Arc::new(to_notes(&smf, &None)?);

        let result = notes
            .iter()
            .fold(Duration::new(0, 0), |acc, n| acc.max(n.start + n.length));

        // result
        Ok(Arc::new(FileInformations {
            duration: Some(result),
        }))
    }
}

/// midi player
pub struct MidiPlayer {
    /// midi connection
    midi_output_connection: Arc<Mutex<MidiOutputConnection>>,

    /// channel to send message to outbox application
    output: Arc<Mutex<Sender<Response>>>,

    /// cancel channel
    cancel: Sender<bool>,

    /// is playing, is the engine is playing a file, this return true
    isplaying: Arc<Mutex<bool>>,

    /// note representation for the display
    notes: Arc<Mutex<Arc<Vec<Note>>>>,
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

fn read_midi_file(
    filename: &PathBuf,
    start_wait: Option<f32>,
) -> Result<(Arc<Vec<Note>>, Ticker, Sheet), Box<dyn Error>> {
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

    Ok((notes, timer, sheet))
}

/// Player trait implementation
impl Player for MidiPlayer {
    fn associated_notes(&self) -> Arc<Mutex<Arc<Vec<Note>>>> {
        Arc::clone(&self.notes)
    }

    fn start_play(
        &mut self,
        filename: &PathBuf,
        start_wait: Option<f32>,
    ) -> Result<(), Box<dyn Error>> {
        // load the midi file
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
        let start_wait_closure = start_wait.clone();

        let notes_access = Arc::clone(&self.notes);

        // thread spawned interpret the Midi event and send them on the line
        thread::spawn(move || {
            if let Err(e) = set_current_thread_priority(ThreadPriority::Max) {
                warn!("fail to set max priority to player thread : {:?}", e);
            }

            let mut buf = Vec::new();
            let mut total_duration = Duration::new(0, 0);
            let mut ticks_counter = 0_u32;

            if let Ok(mut con) = con.lock() {
                debug!("midi connexion aquired");

                all_notes_off(&mut con);

                if let Ok(output_locked) = output_reference.lock() {
                    output_locked
                        .send(Response::CurrentPlayTime(Duration::ZERO))
                        .unwrap();
                }

                let read_result = read_midi_file(&filename_closure, start_wait_closure);

                match read_result {
                    Err(e) => {
                        error!("error in reading file : {:?}", e);
                    }

                    Ok((notes, mut timer, sheet)) => {
                        if let Ok(mut m) = isplaying_info.lock() {
                            *m = true;
                        }
                        {
                            let mut note_guard = notes_access.try_lock().unwrap();
                            *note_guard = Arc::clone(&notes);
                        }

                        // send message
                        if let Ok(output_locked) = output_reference.lock() {
                            let filename = filename_closure.clone();
                            if let Err(err_send_file_started) =
                                output_locked.send(Response::FilePlayStarted((
                                    String::from(filename.to_string_lossy()),
                                    notes,
                                )))
                            {
                                error!(
                                    "error sending start of playing : {:?}",
                                    err_send_file_started
                                );
                            }
                        }

                        let wait_time = if let Some(w) = start_wait {
                            Duration::from_secs_f32(w)
                        } else {
                            Duration::ZERO
                        };

                        if let Some(wait) = start_wait {
                            thread::sleep(Duration::from_secs_f32(wait))
                        }

                        for moment in sheet {
                            if receiver.try_recv().is_ok() {
                                // stopped
                                all_notes_off(&mut con);
                                if let Ok(mut m) = isplaying_info.lock() {
                                    *m = false;
                                }
                                if let Ok(output_locked) = output_reference.lock() {
                                    output_locked.send(Response::FileCancelled).unwrap();
                                }
                                return;
                            }

                            if !moment.is_empty() {
                                if let Ok(mut m) = isplaying_info.lock() {
                                    *m = true;
                                }
                                timer.sleep(ticks_counter);
                                let d = timer.sleep_duration(ticks_counter);
                                total_duration += d;

                                ticks_counter = 0;
                                for event in &moment.events {
                                    match event {
                                        Event::Tempo(val) => timer.change_tempo(*val),

                                        Event::Midi(msg) => {
                                            buf.clear();
                                            let _ = msg.write(&mut buf);
                                            let _ = con.send(&buf);
                                        }
                                        _ => (),
                                    };
                                }

                                if let Ok(output_locked) = output_reference.lock() {
                                    output_locked
                                        .send(Response::CurrentPlayTime(total_duration + wait_time))
                                        .unwrap();
                                }
                            }

                            ticks_counter += 1;
                        }

                        if let Ok(mut m) = isplaying_info.lock() {
                            *m = false;
                        }

                        if let Ok(output_locked) = output_reference.lock() {
                            if let Err(err_send_end_of_file) =
                                output_locked.send(Response::EndOfFile)
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

    fn stop(&mut self) {
        if let Err(e) = self.cancel.send(true) {
            error!("fail to send cancel order : {}", e);
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
}

impl MidiPlayer {
    pub fn new(con: MidiOutputConnection, output: Sender<Response>) -> Self {
        let con = Arc::new(Mutex::new(con));

        let c = channel();

        Self {
            output: Arc::new(Mutex::new(output)),
            cancel: c.0,
            midi_output_connection: con,
            isplaying: Arc::new(Mutex::new(false)),
            notes: Arc::new(Mutex::new(Arc::new(vec![]))),
        }
    }

    fn silence(&self) {
        let mut con = self.midi_output_connection.lock().unwrap();
        let _ = con.send(&[0xb0, 123]);
        let _ = con.send(&[0xb0, 120]);
    }
}

pub fn to_notes(smf: &Smf, start_wait: &Option<f32>) -> Result<Vec<Note>, Box<dyn Error>> {
    let Smf { header, tracks } = smf;
    let miditiming = header.timing;
    let mut timer = Ticker::try_from(miditiming)?;

    debug!("timer : {:?}", &timer);
    timer.change_tempo(DEFAULT_TEMPO_IF_NOT_SET_IN_FILE);

    let sheet = match header.format {
        Format::SingleTrack | Format::Sequential => Sheet::sequential(&tracks),
        Format::Parallel => Sheet::parallel(&tracks),
    };

    let shift_duration = if start_wait.is_some() {
        Duration::from_secs_f32(start_wait.unwrap())
    } else {
        Duration::ZERO
    };

    // note activation
    let mut note_state: Vec<Vec<Duration>> = vec![vec![]; 16 * 128];
    let mut notes: Vec<Note> = vec![];

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
                            if channel_note_vec.len() > 0 {
                                match channel_note_vec.pop() {
                                    Some(d) => notes.push(Note {
                                        channel: uchannel,
                                        note: key,
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
                                if channel_note_vec.len() > 0 {
                                    match channel_note_vec.pop() {
                                        Some(d) => notes.push(Note {
                                            channel: uchannel,
                                            note: key,
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
