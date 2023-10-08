///! this crate create midi from virtual book
///
use nodi::midly::{Header, MidiMessage, Smf, Track, TrackEvent};

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
