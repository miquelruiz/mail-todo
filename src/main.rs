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
use std::thread;
use std::time::Duration;

const MUTT: &'static str = ".mutt";
const CONF: &'static str = "miquelruiz.net";
const ICON: &'static str = "task-due";
const NAME: &'static str = "Mail-todo";
const SLEEP: u64 = 10;

fn main() {
    if gtk::init().is_err() {
        panic!("Failed to initialize GTK");
    }

    let creds = get_credentials();
    let child = thread::Builder::new()
        .name("poller".to_string())
        .spawn(move || { run(creds); }).unwrap();

    let icon = StatusIcon::new_from_icon_name(ICON);
    icon.set_title(NAME);

    let menu = Menu::new();
    let about = MenuItem::new_with_label("About...");
    menu.attach(&about, 0, 1, 0, 1);

    icon.connect_popup_menu(move |_, x, y| {
        println!("Dog science: {} {}", x, y);
        // This seems unimplemented
        // https://github.com/gtk-rs/gtk/blob/d9295b9c776c1b15ec4db0a4025838cb2f92595a/src/auto/menu.rs#L113
        //menu.popup();
    });

    gtk::main();
    let _ = child.join();
}

fn run(creds: Creds) {
    // let creds = get_credentials();
    let mut imap = get_connection(&creds);
    let mut tasks = 0;
    loop {
        let new_tasks = count_tasks(&mut imap);
        if new_tasks != tasks {
            tasks = new_tasks;
            notify(tasks);
        }
        std::thread::sleep(Duration::new(SLEEP, 0));
    }
}

struct Creds {
    user: String,
    pass: String,
    host: String,
    port: u16,
}

fn get_credentials() -> Creds {
    let mut path = match std::env::home_dir() {
        Some(path) => path,
        None => panic!("Can't get home_dir"),
    };

    // Build path to config file
    path.push(MUTT);
    path.push(CONF);

    let content = read_config_file(path.as_path()).unwrap();
    let user = extract_login(r"set imap_user=(\w*)", &content).unwrap();
    let pass = extract_login(r"set imap_pass=(\w*)", &content).unwrap();
    let host = extract_login(r"set folder=imaps?://(.+):\d+", &content).unwrap();
    let port = extract_login(r"set folder=imaps?://.+:(\d+)", &content).unwrap();

    Creds { user: user, pass: pass, host: host, port: port.parse().unwrap() }
}

fn read_config_file(path: &Path) -> Result<String, String> {
    let mut content = String::new();
    let mut file = try!(File::open(&path).map_err(|e| e.to_string()));
    try!(file.read_to_string(&mut content).map_err(|e| e.to_string()));
    Ok(content)
}

fn extract_login(pattern: &str, text: &str) -> Option<String> {
    Regex::new(pattern).ok()
        .and_then(|re| re.captures(text))
        .and_then(|c| c.at(1))
        .map(|i| i.to_string())
}

fn get_connection(creds: &Creds) -> IMAPStream {
    let mut imap_socket = match IMAPStream::connect(
        creds.host.clone(),
        creds.port,
        Some(SslContext::new(SslMethod::Sslv23).unwrap())
    ) {
        Ok(s)  => s,
        Err(e) => panic!("{}", e),
    };

    if let Err(e) = imap_socket.login(&creds.user, &creds.pass) {
        panic!("Error: {}", e)
    };

    imap_socket
}

fn count_tasks(imap_socket: &mut IMAPStream) -> u32 {
    println!("Counting");
    let mbox = match imap_socket.select("ToDo") {
        Ok(m)  => m,
        Err(e) => panic!("Error selecting INBOX: {}", e)
    };
    println!("Found {:?}", mbox.exists);
    mbox.exists
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
