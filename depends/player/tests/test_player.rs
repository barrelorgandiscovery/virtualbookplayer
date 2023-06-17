use player::{midiio::*, PlayerFactory};
use std::{
    convert::TryFrom,
    error::Error,
    fs,
    sync::{
        mpsc::{self, channel, Receiver},
        Arc, Mutex,
    },
    thread, path::PathBuf,
};

#[test]
pub fn test_player() {
    let f = MidiPlayerFactory {
        device_no: 6,
    };

    let (sender, receiver) = channel();
    let (sendercmd, receivercmd) = channel();
   
    thread::spawn(move || {
        let mut p = f.create(sender, receivercmd).unwrap();
        p.play(&PathBuf::from("debussy_63503a_arabesque_2_e_major_(nc)smythe.mid")).unwrap();
    });

    loop {

    }
}
