extern crate libresolv_sys;

use email;
use imap::client::Client;
use openssl::ssl::{SslConnectorBuilder, SslMethod, SslStream};

use ::{Creds, Message, parser, Result, Task};

use std::collections::HashSet;
use std::net::TcpStream;
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::sleep;
use std::time::Duration;


pub fn start(
    creds: Creds,
    folder: &str,
    ui: Sender<Message>,
    wake: Sender<Message>,
    rx: Receiver<Message>,
) {
    let mut slept = 0;
    let mut imap: Option<Client<SslStream<TcpStream>>> = None;

    debug!("Sending 'connect' message");
    let _ = wake.send(Message::Connect);

    while let Ok(m) = rx.recv() { match m {
        Message::Quit => {
            imap.and_then(|mut imap| { imap.logout().ok() });
            break;
        },
        Message::Delete(uid) => if let Some(ref mut imap) = imap {
            delete_task(imap, uid);
            let _ = wake.send(Message::Awake);
        },
        Message::Awake => if let Some(ref mut imap) = imap {
            match get_tasks(imap, &folder) {
                Ok(tasks) => {
                    if let Err(e) = ui.send(Message::Tasks(tasks)) {
                        panic!("Main thread receiver deallocated: {}", e);
                    }
                    debug!("Sending sleep message from awake");
                    let _ = wake.send(Message::Sleep);
                    slept = 0;
                },
                Err(e) => {
                    error!("Error getting tasks: {}", e);
                    // Crap out and reconnect
                    let _ = wake.send(Message::Connect);
                },
            };
        },
        Message::Connect => {
            info!("Setting as disconnected");
            if let Err(e) = ui.send(Message::NotConnected) {
                error!("Couldn't set the status: {}", e);
            }

            imap = match get_connection(&creds) {
                Err(e) => {
                    error!("Error getting connection: {:?}", e);
                    let _ = wake.send(Message::Sleep);
                    None
                },
                Ok(mut imap) => {
                    info!("Connected!");
                    if let Err(e) = ui.send(Message::Connected) {
                        error!("Couldn't set the status: {}", e);
                    }
                    let _ = wake.send(Message::Awake);
                    Some(imap)
                },
            }
        },
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
        m => panic!("Poller received unexpected message! {:?}", m)
    }}
    info!("Exiting poller thread");
}

fn get_connection(creds: &Creds) -> Result<Client<SslStream<TcpStream>>> {
    // Here be dragons.
    // Whenever the thread tries to resolve the mail server domain it will
    // cache the domain name servers used to resolve that. If it happens to try
    // before getting a working internet connection, those nameservers will
    // point to localhost, and surprise surprise, localhost is probably not a
    // dns server.
    // This call ensures that the resolver config is reloaded every single time
    // before trying to connect. That ensures the thread is able to come back
    // from death.
    let _ = unsafe { libresolv_sys::__res_init() };

    debug!("Building ssl stuff");
    let ssl = SslConnectorBuilder::new(SslMethod::tls())?.build();
    debug!("Connecting");
    let mut imap = Client::secure_connect(
        (&creds.host[..], creds.port),
        &creds.host[..],
        ssl,
    )?;
    debug!("Logging in");
    imap.login(&creds.user, &creds.pass)?;
    debug!("Done!");
    Ok(imap)
}

fn get_tasks<T: Read+Write>(
    imap: &mut Client<T>,
    folder: &str,
) -> Result<HashSet<Task>> {
    debug!("Getting tasks");
    let mut tasks: HashSet<Task> = HashSet::new();
    let mbox = imap.select(folder)?;
    for seqn in 1..mbox.exists+1 {
        let seq = &seqn.to_string();
        let uid = get_uid(imap, seq)?;
        match get_subj(imap, seq) {
            Ok(s) => tasks.insert(Task {title: s, uid: uid}),
            Err(e) => { error!("{:?}", e); true },
        };
    }
    debug!("Retrieved tasks: {:?}", tasks);
    Ok(tasks)
}

fn get_uid<T: Read+Write>(imap: &mut Client<T>, seq: &str) -> Result<u64> {
    let resp = imap.fetch(seq, "uid")?;
    let uid = parser::extract_info(r".* FETCH \(UID (\d+)\)", &resp[0])?;
    let uid = uid.parse()?;
    Ok(uid)
}

fn get_subj<T: Read+Write>(imap: &mut Client<T>, seq: &str) -> Result<String> {
    let lines = imap.fetch(seq, "body[header]")?;

    let mut headers = String::new();
    for line in lines {
        headers = headers + &line;
    }

    let mut subject = String::new();
    let subj = parser::extract_info(r"\nSubject: ?(.*?)\r", &headers)?;
    for word in subj.split_whitespace() {
        match email::rfc2047::decode_rfc2047(&word) {
            Some(decoded) => {
                info!("Shit decoded: {:?}", word);
                subject.push_str(&decoded);
            },
            None => {
                subject.push_str(word)
            },
        }
        subject.push(' ');
    }

    Ok(subject)
}

fn delete_task<T: Read+Write>(imap: &mut Client<T>, uid: u64) {
    let _ = imap.uid_store(&format!("{}", uid), "+FLAGS (\\Deleted)");
    let _ = imap.expunge();
}
