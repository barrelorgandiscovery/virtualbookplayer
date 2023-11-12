use player::{midiio::*, PlayerFactory};
use std::{
    convert::TryFrom,
    error::Error,
    fs,
    path::PathBuf,
    sync::{
        mpsc::{self, channel, Receiver},
        Arc, Mutex,
    },
    thread,
};

#[test]
pub fn test_player_informations() {
    let f = MidiPlayerFactory { device_no: 0 };

    let mut getter = f
        .create_information_getter()
        .expect("fail to create the file information constuctor");
    let file = PathBuf::from("debussy_63503a_arabesque_2_e_major_(nc)smythe.mid");

    let result = getter
        .compute(&file)
        .expect("fail to compute file informations");
    println!("file duration : {:?}", result.duration);
}

#[test]
pub fn test_player() {
    let f = MidiPlayerFactory { device_no: 1 };

    let (sender, receiver) = channel();
    let (sendercmd, receivercmd) = channel();

    thread::spawn(move || {
        let mut p = f.create(sender, receivercmd).unwrap();
        p.start_play(&PathBuf::from("autumn_no3_allegro_gp.mid"), None)
            .unwrap();
    });

    loop {
        match receiver.recv().expect("error getting message") {
            player::Response::EndOfFile => {
                println!("End of file received, stopping");
                break;
            }
            player::Response::CurrentPlayTime(t) => {
                println!("current play time :{}", t.as_secs());
            }

            player::Response::FileCancelled => println!("file canceled"),

            player::Response::FilePlayStarted((file, notes)) => {}
        }
    }
}

#[test]
pub fn test_player_1() {
    let f = MidiPlayerFactory { device_no: 0 };

    let (sender, receiver) = channel();
    let (sendercmd, receivercmd) = channel();

    thread::spawn(move || {
        let mut p = f.create(sender, receivercmd).unwrap();
        p.start_play(&PathBuf::from("A PRESENT TU PEUX T EN ALLER.mid"), None)
            .unwrap();
    });

    loop {}
}
