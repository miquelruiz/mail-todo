extern crate libresolv_sys;

use imap::client::Client;
use openssl::ssl::{SslContext, SslMethod, SslStream};

use ::{Creds, Message, parser, Result, Task};

use std::collections::HashSet;
use std::net::TcpStream;
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
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
    let mut tries = 0;
    loop {
        info!("Trying {}:{}... ", creds.host, creds.port);
        if let Err(e) = ui.send(Message::NotConnected) {
            error!("Couldn't set the status: {}", e);
        }
        match get_connection(&creds) {
            Err(e) => {
                error!("Error getting connection: {:?}", e);
                sleep(duration());
            },
            Ok(mut imap) => {
                if let Err(e) = ui.send(Message::Connected) {
                    error!("Couldn't set the status: {}", e);
                }
                if !poll_imap(&mut imap, &ui, &wake, &rx, tries) {
                    info!("Exiting from poller thread");
                    break;
                }
                info!("Coming back from polling");
            },
        };
        sleep(duration());
        tries += 1;
    }
}

fn poll_imap<T: Read+Write>(
    mut imap: &mut Client<T>,
    ui: &Sender<Message>,
    wake: &Sender<Message>,
    rx: &Receiver<Message>,
    tries: u32
) -> bool {
    let mut reconnect = true;
    let wake2 = wake.clone();
    let (awake_tx, awake_rx) = channel::<Message>();

    debug!("Spawning awakener thread");
    let handler = thread::Builder::new()
        .name(format!("awakener{}", tries))
        .spawn(move || loop {
            if let Ok(m) = awake_rx.try_recv() { match m {
                Message::Quit => {
                    debug!("awakener{} exits", tries);
                    break;
                },
                m => panic!("Awakener received unexpected message! {:?}", m)
            }}

            debug!("Sending awake message from awakener{}", tries);
            let _ = wake2.send(Message::Awake);

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
            let _ = wake.send(Message::Awake);
        },
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
        m => panic!("Poller received unexpected message! {:?}", m)
    }}

    // Stop the awakener only if we are going to reconnect so it's not leaked
    if reconnect {
        // We are exiting, so tell our awakener to exit too
        if let Err(e) = awake_tx.send(Message::Quit) {
            warn!("awakener{} thread possibly leaked! {:?}", tries, e);
        }
        debug!("Waiting for awakener thread to finish");
        if let Err(e) = handler.join() {
            error!("awakener{} panic'ed: {:?}", tries, e)
        }
    }
    info!("Exiting poll_imap");
    reconnect
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
    let ssl = SslContext::new(SslMethod::Sslv23)?;
    debug!("Connecting");
    let mut imap = Client::secure_connect(
        (&creds.host[..], creds.port),
        ssl,
    )?;
    debug!("Logging in");
    imap.login(&creds.user, &creds.pass)?;
    debug!("Done!");
    Ok(imap)
}

fn get_tasks<T: Read+Write>(mut imap: &mut Client<T>) -> Result<HashSet<Task>> {
    debug!("Getting tasks");
    let mut tasks: HashSet<Task> = HashSet::new();
    let mbox = imap.select(::MBOX)?;
    for seqn in 1..mbox.exists+1 {
        let seq = &seqn.to_string();
        let uid = get_uid(imap, seq)?;
        let subj = get_subj(imap, seq)?;
        tasks.insert(Task {title: subj, uid: uid});
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

    let subj = parser::extract_info(r"Subject: (.*)\r", &headers)?;
    Ok(subj)
}

fn delete_task<T: Read+Write>(imap: &mut Client<T>, uid: u64) {
    let _ = imap.uid_store(&format!("{}", uid), "+FLAGS (\\Deleted)");
    let _ = imap.expunge();
}
