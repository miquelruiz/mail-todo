extern crate gtk;
use gtk::prelude::*;
use gtk::{Menu, MenuItem, StatusIcon};

extern crate imap;
use imap::client::IMAPStream;

extern crate notify_rust;
use notify_rust::Notification;

extern crate openssl;
use openssl::ssl::{SslContext, SslMethod};

extern crate regex;
use regex::Regex;

use std::io::prelude::*;
use std::fs::File;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

const MUTT: &'static str = ".mutt";
const CONF: &'static str = "miquelruiz.net";
const ICON: &'static str = "task-due";
const NAME: &'static str = "Mail-todo";
const MBOX: &'static str = "ToDo";
const SLEEP: u64 = 10;

fn main() {
    if gtk::init().is_err() {
        panic!("Failed to initialize GTK");
    }

    // Channel used to hide/show the status icon
    let (tx, rx) = channel::<Message>();

    let creds = get_credentials().unwrap();
    let child = thread::Builder::new()
        .name("poller".to_string())
        .spawn(move || { poll_imap(creds, rx); }).unwrap();

    let icon = StatusIcon::new_from_icon_name(ICON);
    icon.set_title(NAME);

    icon.connect_popup_menu(move |_, x, y| {
        println!("Dog science: {} {}", x, y);
        let _ = tx.send(Message::Quit).unwrap();
        gtk::main_quit();
    });

    gtk::main();
    let _ = child.join();
}

fn poll_imap(creds: Creds, rx: Receiver<Message>) {
    loop {
        println!("Trying {}:{}... ", creds.host, creds.port);
        match get_connection(&creds) {
            Err(e) => {
                println!("  {}", e);
                std::thread::sleep(Duration::new(SLEEP, 0));
            },
            Ok(mut imap) => {
                println!("Connected!");
                let mut tasks = 0;
                loop {
                    match count_tasks(&mut imap) {
                        Err(e) => { println!("{}", e); break },
                        Ok(t)  => if t != tasks { tasks = t; notify(tasks); },
                    }
                    match rx.try_recv() {
                        Ok(Message::Quit) => return,
                        Err(_) => (),
                    }
                    std::thread::sleep(Duration::new(SLEEP, 0));
                }
            },
        };
        std::thread::sleep(Duration::new(SLEEP, 0));
    }
}

enum Message {
    Quit,
}

struct Creds {
    user: String,
    pass: String,
    host: String,
    port: u16,
}

fn get_credentials() -> Result<Creds, String> {
    let mut path = try!(std::env::home_dir().ok_or("Can't get home dir"));

    // Build path to config file
    path.push(MUTT);
    path.push(CONF);

    let content = try!(read_config_file(path.as_path()));
    let user = try!(extract_login(r"set imap_user=(\w*)", &content));
    let pass = try!(extract_login(r"set imap_pass=(\w*)", &content));
    let host = try!(extract_login(r"set folder=imaps?://(.+):\d+", &content));
    let port = try!(extract_login(r"set folder=imaps?://.+:(\d+)", &content));

    port.parse()
        .map_err(|e :std::num::ParseIntError| e.to_string())
        .and_then(|p| Ok(Creds {
            user: user,
            pass: pass,
            host: host,
            port: p,
        }))
}

fn read_config_file(path: &Path) -> Result<String, String> {
    let mut content = String::new();
    let mut file = try!(File::open(&path).map_err(|e| e.to_string()));
    try!(file.read_to_string(&mut content).map_err(|e| e.to_string()));
    Ok(content)
}

fn extract_login(pattern: &str, text: &str) -> Result<String, String> {
    Regex::new(pattern).map_err(|e| e.to_string())
        .and_then(|re| re.captures(text).ok_or(String::from("Couldn't match")))
        .and_then(|c| c.at(1).ok_or(String::from("No captures")))
        .map(|i| i.to_string())
}

fn get_connection(creds: &Creds) -> Result<IMAPStream, String> {
    let mut imap_socket = try!(IMAPStream::connect(
        creds.host.clone(),
        creds.port,
        SslContext::new(SslMethod::Sslv23).ok()
    ).map_err(|e| e.to_string()));

    try!(imap_socket.login(&creds.user, &creds.pass)
        .map_err(|e| e.to_string()));

    Ok(imap_socket)
}

fn count_tasks(imap_socket: &mut IMAPStream) -> Result<u32, String> {
    print!("Counting... ");
    imap_socket.select(MBOX)
        .map_err(|e| format!("Error selecting mbox: {}", e))
        .map(|m| { println!("found {:?}", m.exists); m.exists })
}

#[allow(dead_code)]
fn logout(imap_socket: &mut IMAPStream) {
    if let Err(e) = imap_socket.logout() {
        println!("Error {}", e)
    };
}

fn notify(tasks: u32) {
    println!("{:?} pending tasks", tasks);
    Notification::new()
        .summary("Notifier")
        .body(&format!("{} tasks pending", tasks))
        .icon("task-due")
        .timeout(5000)
        .show().unwrap();
}
