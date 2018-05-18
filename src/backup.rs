use Message;

use std::sync::mpsc::{Receiver, Sender};
use std::thread::sleep;
use std::time::Duration;

pub fn start(wake: Sender<Message>, rx: Receiver<Message>) {
    let mut slept = 0;

    debug!("Sending first awake message to backup thread");
    let _ = wake.send(Message::Awake);

    while let Ok(m) = rx.recv() {
        match m {
            Message::Quit => break,
            Message::Awake => {
                info!("Awaken");

                info!("Sending sleep message from awake");
                let _ = wake.send(Message::Sleep);
            }
            Message::Sleep => {
                sleep(Duration::new(1, 0));
                slept += 1;
                if slept >= ::SLEEP {
                    let _ = wake.send(Message::Awake);
                    slept = 0;
                } else {
                    let _ = wake.send(Message::Sleep);
                }
            }
            m => panic!(
                "Backup thread received an unexpected message! {:?}",
                m
            ),
        }
    }

    info!("Exiting backup thread");
}
