///! this crate create midi from virtual book
use std::{
    collections::HashMap,
    error::Error,
    io::{Read, Write},
};

use bookparsing::{Hole, ScaleDefinition};

use nodi::midly::{Header, MidiMessage, Smf, Track, TrackEvent};
extern crate serde;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Mapping {
    // track: i16,
    midi_channel: i8,
    note: u8,
    modifier: Option<Modifier>,
}

/// intermediate event type
pub enum EventType {
    ACTIVATE,
    DEACTIVATE,
}

/// intermediate hole associated event (for midi generation)
/// this explode the hole into start/stop events (needed for midi output)
pub struct HoleEvent {
    timestamp: i64,
    channel: u8,
    note: u8,
    event_type: EventType,
}

impl HoleEvent {
    /// Hole Event constructor using the needed elements
    #[allow(unused)]
    pub fn from(timestamp: i64, channel: u8, note: u8, event_type: EventType) -> HoleEvent {
        HoleEvent {
            timestamp,
            channel,
            note,
            event_type,
        }
    }
    /// construct a HoleEvent from a hole
    pub fn from_hole(hole: &Hole, mapping: &Mapping) -> (HoleEvent, HoleEvent) {
        (
            HoleEvent {
                timestamp: hole.timestamp,
                channel: mapping.midi_channel as u8,
                note: mapping.note,
                event_type: EventType::ACTIVATE,
            },
            HoleEvent {
                timestamp: hole.timestamp + hole.length,
                channel: mapping.midi_channel as u8,
                note: mapping.note,
                event_type: EventType::DEACTIVATE,
            },
        )
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum Modifier {
    MECHANICAL_READ,
    PERCUSSION_TRIGGERED_AT_END,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ConversionParameters {
    read_size: f32, // read lag from
}

/// conversion structure holding converting books into midi files
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Conversion {
    /// name of the conversion
    name: String,
    global_parameters: Option<ConversionParameters>,
    mapping: HashMap<u16, Mapping>,
}

impl Conversion {
    pub fn convert(&self, hole: &Hole) -> Vec<HoleEvent> {
        if let Some(m) = self.mapping.get(&hole.track) {
            let result = HoleEvent::from_hole(hole, m);
            return vec![result.0, result.1];
        }

        vec![]
    }
}

/// this function parse a string defining a midi code (ex : A3, B7 ...)
pub fn parse_note(s: &str) -> Result<u8, Box<dyn Error>> {
    let mut notes = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    notes.reverse(); // for check (diese must be checked first)

    if s.is_empty() {
        return Err("empty string given".into());
    }

    for (index, i) in notes.iter().enumerate() {
        if s.starts_with(i) {
            let octave: i8 = s[i.len()..].parse::<i8>()?;
            return Ok((12 * (octave + 1) + ((11 - index) as i8)) as u8);
        }
    }
    Err(format!("note {}, not found", &s).into())
}

#[test]
pub fn test_parse_note() {
    assert_eq!(parse_note("C0".into()).unwrap(), 12);
    assert_eq!(parse_note("C#0".into()).unwrap(), 13);
    assert_eq!(parse_note("A4".into()).unwrap(), 69);
    assert_eq!(parse_note("C-1".into()).unwrap(), 0);
    assert_eq!(parse_note("G9".into()).unwrap(), 127);
    // diese test
    assert_eq!(parse_note("F#9".into()).unwrap(), 126);
}

/// create an automatic conversion from the scale definition
/// this function map the notes to channel 0
pub fn create_conversion_from_scale(scale: &ScaleDefinition) -> Result<Conversion, Box<dyn Error>> {
    let mut mapping: HashMap<u16, Mapping> = HashMap::new();

    for t in &scale.tracks.tracks {
        // pub enum Track {
        //     Unknown,
        //     TrackNoteDef(TrackNote),
        //     TrackDrumDef(TrackDrum),
        //     TrackRegisterControlStartDef(TrackRegisterControlStart),
        //     TrackRegisterControlResetDef(TrackRegisterControlReset),
        // }

        match t {
            bookparsing::Track::TrackNoteDef(n) => {
                mapping.insert(
                    n.no,
                    Mapping {
                        midi_channel: 0,
                        note: parse_note(n.note.as_str())? - 12,
                        modifier: Some(Modifier::MECHANICAL_READ),
                    },
                );
            }
            bookparsing::Track::TrackDrumDef(d) => {
                mapping.insert(
                    d.no,
                    Mapping {
                        midi_channel: 9,
                        note: 45, // FIXME // parse_note(n.note.as_str())?,
                        modifier: Some(Modifier::MECHANICAL_READ),
                    },
                );
            }

            _ => {}
        }
    }

    let result = Conversion {
        global_parameters: None,
        name: "automatic conversion".into(),
        mapping,
    };

    Ok(result)
}

/// write a conversion to a stream
#[allow(unused)]
pub fn write_conversion(
    conversion: &Conversion,
    writer: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    serde_yaml::to_writer(writer, conversion)?;
    Ok(())
}

/// test reading conversion
#[test]
pub fn test_write_conversion_in_file() -> Result<(), Box<dyn Error>> {
    let f = File::create("write_test.yaml")?;
    let mut buf_writer = BufWriter::new(f);

    let mut h: HashMap<u16, Mapping> = HashMap::new();
    h.insert(
        0,
        Mapping {
            midi_channel: 0,
            note: 69,
            modifier: Some(Modifier::MECHANICAL_READ),
        },
    );

    let conversion = Conversion {
        name: "my transformation".into(),
        global_parameters: Some(ConversionParameters { read_size: 2.0 }),
        mapping: h,
    };

    write_conversion(&conversion, &mut buf_writer)
}

/// read a conversion from stream
pub fn read_conversion(reader: &mut dyn Read) -> Result<Conversion, Box<dyn Error>> {
    let result = serde_yaml::from_reader(reader)?;

    Ok(result)
}

/// convert a book using a given conversion
pub fn convert<'a>(
    book: &bookparsing::VirtualBook,
    conversion: &Conversion,
) -> Result<Smf<'a>, Box<dyn Error>> {
    let all_holes = &book.holes;

    // shift the result when there are some negative elements in the book
    let smallest = all_holes
        .holes
        .iter()
        .fold(0, |a, e| if e.timestamp < a { e.timestamp } else { a });

    let all_events = all_holes
        .holes
        .iter()
        .filter(|hole| hole.length > 0)
        .map(|h| Hole {
            timestamp: h.timestamp - smallest,
            length: h.length,
            track: h.track,
        })
        .flat_map(|h| conversion.convert(&h));

    let mut sorted_result: Box<Vec<HoleEvent>> = Box::new(all_events.collect());
    sorted_result.sort_by_key(|e| e.timestamp);

    const TICKS_PER_BEAT: u16 = 10_000;
    let mut smf = Smf::new(Header {
        format: nodi::midly::Format::SingleTrack {},
        timing: nodi::midly::Timing::Metrical(TICKS_PER_BEAT.into()),
    });

    let midi_events = sorted_result
        .iter()
        .fold((0, Box::<Vec<TrackEvent<'_>>>::default()), |t, e| {
            let mut v = t.1;

            let delta_in_microseconds: i64 = e.timestamp - t.0;
            assert!(delta_in_microseconds >= 0);
            let delta_in_ticks: u32 =
                (  (delta_in_microseconds as f64 * TICKS_PER_BEAT as f64) / 1_000_000.0 * 120.0 / 60.0   ) as u32;
            v.push(TrackEvent {
                delta: delta_in_ticks.into(),
                kind: nodi::midly::TrackEventKind::Midi {
                    channel: e.channel.into(),
                    message: match e.event_type {
                        EventType::ACTIVATE => MidiMessage::NoteOn {
                            key: e.note.into(),
                            vel: 127.into(),
                        },
                        EventType::DEACTIVATE => MidiMessage::NoteOff {
                            key: e.note.into(),
                            vel: 127.into(),
                        },
                    },
                },
            });

            (e.timestamp, v)
        })
        .1;

    let mut t = Track::new();
    t.extend(*midi_events);

    smf.tracks.push(t);

    Ok(smf)
}

/// sample for using a conversion
#[test]
pub fn test_conversion() -> Result<(), Box<dyn Error>> {
    let c = Conversion {
        mapping: HashMap::new(),
        name: String::from("conversion"),
        global_parameters: None,
    };

    let f = File::open("test_save.book")?;
    let mut reader = BufReader::new(f);
    let _vb: VirtualBook = bookparsing::read_book_stream(&mut reader)?;

    let start_time = Instant::now();
    let result = convert(&_vb, &c).expect("error in conversion");
    print!(
        "result : {:?}, in {}",
        result,
        (Instant::now() - start_time).as_secs_f32()
    );
    Ok(())
}

/// test the midi creation
#[test]
pub fn test_create_midi() {
    let mut midi = Smf::new(Header {
        format: nodi::midly::Format::SingleTrack {},
        timing: nodi::midly::Timing::Metrical(5000.into()),
    });

    let mut t = Track::new();
    let noteon = TrackEvent {
        delta: 0.into(),
        kind: nodi::midly::TrackEventKind::Midi {
            channel: 0.into(),
            message: MidiMessage::NoteOn {
                key: 69.into(),
                vel: 127.into(),
            },
        },
    };
    t.push(noteon);

    let noteoff = TrackEvent {
        delta: 10_000.into(),
        kind: nodi::midly::TrackEventKind::Midi {
            channel: 0.into(),
            message: MidiMessage::NoteOff {
                key: 69.into(),
                vel: 127.into(),
            },
        },
    };
    t.push(noteoff);

    midi.tracks.push(t);

    midi.save("test.mid").unwrap();
}
