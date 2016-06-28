use imap::client::IMAPStream;
use openssl::ssl::{SslContext, SslMethod};

use ::parser::Creds;
use ::{Message, parser, Result, Task};

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::sleep;
use std::time::Duration;

fn duration() -> Duration { Duration::new(::SLEEP, 0) }

pub fn connect(creds: Creds, tx: Sender<Message>, rx: Receiver<Message>) {
    loop {
        println!("Trying {}:{}... ", creds.host, creds.port);
        tx.send(Message::Status("Connecting..."));
        match get_connection(&creds) {
            Err(e) => {
                println!("  {:?}", e);
                sleep(duration());
            },
            Ok(mut imap) => {
                tx.send(Message::Status("Connected"));
                poll_imap(&mut imap, &tx, &rx);
                break;
            },
        };
        sleep(duration());
    }
}

fn poll_imap(
    mut imap: &mut IMAPStream,
    tx: &Sender<Message>,
    rx: &Receiver<Message>
) {
    loop {
        match get_tasks(&mut imap) {
            Ok(tasks) => { tx.send(Message::Tasks(tasks)); },
            Err(e) => println!("Error getting tasks: {}", e),
        }

        if let Ok(Message::Quit) = rx.try_recv() {
            // Since we are exiting, no big deal if it fails
            let _ = imap.logout();
            break;
        }
        sleep(duration());
    }
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
