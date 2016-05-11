extern crate imap;
extern crate openssl;
extern crate regex;

use openssl::ssl::{SslContext, SslMethod};
use imap::client::{IMAPStream, IMAPMailbox};
use regex::Regex;

use std::error::Error;
use std::io::prelude::*;
use std::fs::File;
use std::path::Path;

const MUTT: &'static str = ".mutt";
const CONF: &'static str = "miquelruiz.net";

fn main() {
    println!("This is mail-todo");
//    let tasks = count_tasks(get_credentials());
    let creds = get_credentials();
    let tasks = count_tasks(creds);
    println!("{} tasks pending for today", tasks);
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

    // Build path to config file and make immutable
    path.push(MUTT);
    path.push(CONF);
    let path = path.as_path();

    let content = read_config_file(path);
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

fn count_tasks(creds: Creds) -> u32 {
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

//    match imap_socket.fetch("1", "body[header]") {
//        Ok(lines) => {
//            for line in lines.iter() {
//                print!("{}", line)
//            }
//        },
//        Err(e) => panic!("Error fetching mail: {}", e),
//    }

    if let Err(e) = imap_socket.logout() {
        println!("Error {}", e)
    };

    mbox.exists
}
