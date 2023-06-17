use std::{path::PathBuf, time::Duration};

use core::result::Result;
use std::error::Error;

use nodi::{
    midly::{Format, MidiMessage, Smf},
    timers::Ticker,
    Event, MidiEvent, Sheet, Timer,
};
use player::midiio::to_notes;

#[test]
pub fn test_reading_midifile() -> Result<(), Box<dyn Error>> {
    let filename = &PathBuf::from("chopin-nocturne-op9-no2.mid");

    // load the midi file
    let file_content_data = std::fs::read(filename)?;

    // Load bytes first
    let smf = Smf::parse(&file_content_data)?;
    let notes = to_notes(&smf)?;

    println!("notes : {:?}", notes);

    Ok(())
}

#[test]
pub fn test_tracks() -> Result<(), Box<dyn Error>> {
    let filename = &PathBuf::from("chopin-nocturne-op9-no2.mid");

    // load the midi file
    let file_content_data = std::fs::read(filename)?;

    // Load bytes first
    let smf = Smf::parse(&file_content_data)?;

    let tracks = smf.tracks;
    println!("{:?}", &tracks);

    Ok(())
}
