extern crate imap;
use imap::client::IMAPStream;

extern crate notify_rust;
use notify_rust::Notification;

extern crate openssl;
use openssl::ssl::{SslContext, SslMethod};

extern crate regex;
use regex::Regex;

use std::error::Error;
use std::io::prelude::*;
use std::fs::File;
use std::path::Path;
use std::thread;
use std::time::Duration;

const MUTT: &'static str = ".mutt";
const CONF: &'static str = "miquelruiz.net";

fn main() {
    let child = thread::spawn(move || {run()});
    let res = child.join();
}

fn run() {
    let creds = get_credentials();
    loop {
        let tasks = count_tasks(&creds);
        Notification::new()
            .summary("Notifier")
            .body(&format!("{} tasks pending", tasks))
            .icon("task-due")
            .timeout(5000)
            .show().unwrap();
        std::thread::sleep(Duration::new(10, 0));
    }
}

struct Creds {
    user: String,
    pass: String,
}

fn get_credentials() -> Creds {
    let mut path = match std::env::home_dir() {
        Some(path) => path,
        None => panic!("Can't get home_dir"),
    };

    // Build path to config file
    path.push(MUTT);
    path.push(CONF);

    let content = read_config_file(path.as_path());
    let user = extract_login(r"set imap_user=(\w*)", &content);
    let pass = extract_login(r"set imap_pass=(\w*)", &content);

    Creds { user: user, pass: pass }
}

fn read_config_file(path: &Path) -> String {
    // if it's not mutable, read_to_string crashes
    let mut file = match File::open(&path) {
        Err(why) => panic!("Can't open {}: {}",
            path.display(), Error::description(&why)),
        Ok(file) => file,
    };

    // Alternatively if this function returned Result<T, E>
    //let mut file = try!(File::open(&path));

    let mut content = String::new();
    if let Err(why) = file.read_to_string(&mut content) {
        panic!("Can't read {}: {}", path.display(), Error::description(&why))
    };

    content
}

fn extract_login(pattern: &str, text: &str) -> String {
    let re = Regex::new(pattern).unwrap();
    let caps = re.captures(text).unwrap();
    let info = match caps.at(1) {
        Some(info) => info,
        None => panic!("Couldn't match the regexp {} against {}", re, text),
    };
    info.to_string()
}

fn count_tasks(creds: &Creds) -> u32 {
    let mut imap_socket = match IMAPStream::connect(
        "mail.miquelruiz.net",
        993,
        Some(SslContext::new(SslMethod::Sslv23).unwrap())
    ) {
        Ok(s)  => s,
        Err(e) => panic!("{}", e),
    };

    if let Err(e) = imap_socket.login(&creds.user, &creds.pass) {
        panic!("Error: {}", e)
    };

    let mbox = match imap_socket.select("ToDo") {
        Ok(m)  => m,
        Err(e) => panic!("Error selecting INBOX: {}", e)
    };

    if let Err(e) = imap_socket.logout() {
        println!("Error {}", e)
    };

    mbox.exists
}
