use imap::client::IMAPStream;
use openssl::ssl::{SslContext, SslMethod};

use ::{Creds, Message, parser, Result, Task};

use std::collections::HashSet;
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
        println!("Trying {}:{}... ", creds.host, creds.port);
        if let Err(e) = ui.send(Message::Status("Connecting...")) {
            println!("Couldn't set the status: {}", e);
        }
        match get_connection(&creds) {
            Err(e) => {
                println!("  {:?}", e);
                sleep(duration());
            },
            Ok(mut imap) => {
                if let Err(e) = ui.send(Message::Status("Connected")) {
                    println!("Couldn't set the status: {}", e);
                }
                poll_imap(&mut imap, ui, wake, rx);
                break;
            },
        };
        sleep(duration());
    }
}

fn poll_imap(
    mut imap: &mut IMAPStream,
    ui: Sender<Message>,
    wake: Sender<Message>,
    rx: Receiver<Message>
) {
    let child = thread::Builder::new()
        .name("awakener".to_string())
        .spawn(move || loop { wake.send(Message::Awake); sleep(duration()); })
        .unwrap();

    while let Ok(m) = rx.recv() { match m {
        Message::Quit => { let _ = imap.logout(); break; },
        Message::Delete(uid) => println!("Should delete {}", uid),
        Message::Awake => match get_tasks(&mut imap) {
            Ok(tasks) => { if let Err(e) = ui.send(Message::Tasks(tasks)) {
                panic!("Main thread receiver deallocated: {}", e);
            }},
            Err(e) => println!("Error getting tasks: {}", e),
        },
        m => panic!("Received unexpected message! {:?}", m)
    }}
}

fn get_connection(creds: &Creds) -> Result<IMAPStream> {
    let mut imap = try!(IMAPStream::connect(
        (&creds.host[..], creds.port),
        SslContext::new(SslMethod::Sslv23).ok()
    ));
    try!(imap.login(&creds.user, &creds.pass));
    Ok(imap)
}

fn get_tasks(mut imap: &mut IMAPStream) -> Result<HashSet<Task>> {
    let mut tasks: HashSet<Task> = HashSet::new();
    let mbox = try!(imap.select(::MBOX));
    for seqn in 1..mbox.exists+1 {
        let seq = &seqn.to_string();
        let uid = try!(get_uid(imap, seq));
        let subj = try!(get_subj(imap, seq));
        tasks.insert(Task {title: subj, uid: uid});
    }
//    println!("{:?}", tasks);
    Ok(tasks)
}

fn get_uid(imap: &mut IMAPStream, seq: &str) -> Result<u64> {
    let resp = try!(imap.fetch(seq, "uid"));
    let uid = try!(parser::extract_info(r".* FETCH \(UID (\d+)\)", &resp[0]));
    let uid = try!(uid.parse());
    Ok(uid)
}

fn get_subj(imap: &mut IMAPStream, seq: &str) -> Result<String> {
    let lines = try!(imap.fetch(seq, "body[header]"));

    let mut headers = String::new();
    for line in lines {
        headers = headers + &line;
    }

    let subj = try!(parser::extract_info(r"Subject: (.*)\r", &headers));
    Ok(subj)
}
