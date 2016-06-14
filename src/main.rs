extern crate gtk;
use gtk::prelude::*;
use gtk::{Builder, CheckButton, ListBox, StatusIcon, Window};

extern crate glib;

extern crate imap;
use imap::client::IMAPStream;

extern crate notify_rust;
use notify_rust::Notification;

extern crate openssl;
use openssl::ssl::{SslContext, SslMethod};

extern crate regex;
use regex::Regex;

use std::cell::RefCell;
use std::io::prelude::*;
use std::fs::File;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

const MUTT: &'static str = ".mutt";
const CONF: &'static str = "miquelruiz.net";
const ICON: &'static str = "task-due";
const NAME: &'static str = "Mail-todo";
const MBOX: &'static str = "ToDo";
const SLEEP: u64 = 10;

thread_local!(
    static GLOBAL: RefCell<Option<(gtk::ListBox, Receiver<String>)>> =
        RefCell::new(None)
);

fn main() {
    if gtk::init().is_err() {
        panic!("Failed to initialize GTK");
    }

    let (stoptx, stoprx) = channel::<Message>();
    let (todotx, todorx) = channel::<String>();

    let icon = StatusIcon::new_from_icon_name(ICON);
    icon.set_title(NAME);
//    icon.connect_popup_menu(move |_, x, y| {
//        println!("Dog science: {} {}", x, y);
//        window.show_all();
//    });

    let ui = include_str!("test.glade");
    let builder = Builder::new_from_string(ui);
    let window: Window = builder.get_object("window").unwrap();
    window.connect_delete_event(move |_, _| {
        println!("Closing...");
        let _ = stoptx.send(Message::Quit).unwrap();
        icon.set_visible(false);
        gtk::main_quit();
        Inhibit(false)
    });

    let content: ListBox = builder.get_object("content").unwrap();

    GLOBAL.with(move |global| {
        *global.borrow_mut() = Some((content, todorx))
    });

    let creds = get_credentials().unwrap();
    let child = thread::Builder::new()
        .name("poller".to_string())
        .spawn(move || {
            glib::timeout_add_seconds(SLEEP as u32, receive);
            poll_imap(creds, todotx, stoprx);
        }).unwrap();

    window.show_all();
    gtk::main();
    let _ = child.join();
}

fn receive() -> glib::Continue {
    GLOBAL.with(|global| {
        if let Some((ref lb, ref rx)) = *global.borrow() {
            if let Ok(todo) = rx.try_recv() {
                let check = CheckButton::new_with_label(&todo);
                lb.add(&check);
                lb.show_all();
            }
        }
    });
    glib::Continue(true)
}

fn poll_imap(creds: Creds, tx: Sender<String>, rx: Receiver<Message>) {
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
                        Ok(t)  => if t != tasks {
                            tasks = t;
                            notify(tasks);
                            let _ = tx.send("ZOMFG!".to_string());
                        },
                    }
                    match rx.try_recv() {
                        Ok(Message::Quit) => {
                            // Since we are exiting, no big deal if it fails
                            let _ = imap.logout();
                            return;
                        },
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
    imap_socket.select(MBOX)
        .map_err(|e| format!("Error selecting mbox: {}", e))
        .map(|m| m.exists)
}

fn notify(tasks: u32) {
    println!("{:?} pending tasks", tasks);
    Notification::new()
        .summary(NAME)
        .body(&format!("{} tasks pending", tasks))
        .icon(ICON)
        .timeout(5000)
        .show().unwrap();
}
