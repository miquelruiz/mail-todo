use imap::client::Client;
use openssl::ssl::{SslContext, SslMethod, SslStream};

use ::{Creds, Message, parser, Result, Task};

use std::collections::HashSet;
use std::net::TcpStream;
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::sleep;
use std::time::Duration;


fn duration() -> Duration { Duration::new(::SLEEP, 0) }

pub fn connect(
    creds: Creds,
    ui: Sender<Message>,
    wake: Sender<Message>,
    rx: Receiver<Message>
) {
    loop {
        info!("Trying {}:{}... ", creds.host, creds.port);
        if let Err(e) = ui.send(Message::Status("Connecting...")) {
            error!("Couldn't set the status: {}", e);
        }
        match get_connection(&creds) {
            Err(e) => {
                error!("Error getting connection: {:?}", e);
                sleep(duration());
            },
            Ok(mut imap) => {
                if let Err(e) = ui.send(Message::Status("Connected")) {
                    error!("Couldn't set the status: {}", e);
                }
                if !poll_imap(&mut imap, &ui, &wake, &rx) {
                    info!("Exiting from poller thread");
                    break;
                }
                info!("Coming back from polling");
            },
        };
        sleep(duration());
    }
}

fn poll_imap<T: Read+Write>(
    mut imap: &mut Client<T>,
    ui: &Sender<Message>,
    wake: &Sender<Message>,
    rx: &Receiver<Message>
) -> bool {
    let mut reconnect = true;
    let wake = wake.clone();
    let _ = thread::Builder::new()
        .name("awakener".to_string())
        .spawn(move || loop {
            let _ = wake.send(Message::Awake);
            sleep(duration());
        })
        .unwrap();

    while let Ok(m) = rx.recv() { match m {
        Message::Quit => {
            reconnect = false;
            let _ = imap.logout();
            break;
        },
        Message::Delete(uid) => {
            delete_task(&mut imap, uid);
        }
        Message::Awake => match get_tasks(&mut imap) {
            Ok(tasks) => { if let Err(e) = ui.send(Message::Tasks(tasks)) {
                panic!("Main thread receiver deallocated: {}", e);
            }},
            Err(e) => {
                error!("Error getting tasks: {}", e);
                // If something goes wrong, crap out and force reconnection
                break;
            },
        },
        m => panic!("Received unexpected message! {:?}", m)
    }}
    info!("Exiting poll_imap");
    reconnect
}

fn get_connection(creds: &Creds) -> Result<Client<SslStream<TcpStream>>> {
    let ssl = try!(SslContext::new(SslMethod::Sslv23));
    let mut imap = try!(Client::secure_connect(
        (&creds.host[..], creds.port),
        ssl,
    ));
    try!(imap.login(&creds.user, &creds.pass));
    Ok(imap)
}

fn get_tasks<T: Read+Write>(mut imap: &mut Client<T>) -> Result<HashSet<Task>> {
    let mut tasks: HashSet<Task> = HashSet::new();
    let mbox = try!(imap.select(::MBOX));
    for seqn in 1..mbox.exists+1 {
        let seq = &seqn.to_string();
        let uid = try!(get_uid(imap, seq));
        let subj = try!(get_subj(imap, seq));
        tasks.insert(Task {title: subj, uid: uid});
    }
    debug!("{:?}", tasks);
    Ok(tasks)
}

fn get_uid<T: Read+Write>(imap: &mut Client<T>, seq: &str) -> Result<u64> {
    let resp = try!(imap.fetch(seq, "uid"));
    let uid = try!(parser::extract_info(r".* FETCH \(UID (\d+)\)", &resp[0]));
    let uid = try!(uid.parse());
    Ok(uid)
}

fn get_subj<T: Read+Write>(imap: &mut Client<T>, seq: &str) -> Result<String> {
    let lines = try!(imap.fetch(seq, "body[header]"));

    let mut headers = String::new();
    for line in lines {
        headers = headers + &line;
    }

    let subj = try!(parser::extract_info(r"Subject: (.*)\r", &headers));
    Ok(subj)
}

fn delete_task<T: Read+Write>(imap: &mut Client<T>, uid: u64) {
    let _ = imap.uid_store(&format!("{}", uid), "+FLAGS (\\Deleted)");
    let _ = imap.expunge();
}
