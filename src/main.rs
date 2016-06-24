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
use std::collections::HashSet;
use std::error::Error;
use std::io::prelude::*;
use std::fs::File;
use std::path::Path;
use std::result;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

const MUTT: &'static str = ".mutt";
const CONF: &'static str = "miquelruiz.net";
const ICON: &'static str = "task-due";
const NAME: &'static str = "Mail-todo";
const MBOX: &'static str = "ToDo";
const SLEEP: u64 = 10;

type Result<T> = result::Result<T, Box<Error>>;

enum Message {
    Quit,
}

#[derive(Debug)]
struct Creds {
    user: String,
    pass: String,
    host: String,
    port: u16,
}

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
struct Task {
    title: String,
    uid: u64,
}

thread_local!(
    static GLOBAL: RefCell<
        Option<(gtk::Builder, Receiver<HashSet<Task>>, HashSet<Task>)>
    > = RefCell::new(None)
);

fn main() {
    if gtk::init().is_err() {
        panic!("Failed to initialize GTK");
    }

    let (stoptx, stoprx) = channel::<Message>();
    let (todotx, todorx) = channel::<HashSet<Task>>();

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

    let todo: HashSet<Task> = HashSet::new();

    GLOBAL.with(move |global| {
        *global.borrow_mut() = Some((builder, todorx, todo))
    });

    let creds = get_credentials().unwrap();
    let child = thread::Builder::new()
        .name("poller".to_string())
        .spawn(move || {
            glib::timeout_add(100, receive);
            connect(creds, todotx, stoprx);
        }).unwrap();

    window.show_all();
    gtk::main();
    let _ = child.join();
}

fn receive() -> glib::Continue {
    GLOBAL.with(|global| {
        if let Some((ref ui, ref rx, ref mut todo)) = *global.borrow_mut() {
//            let mut notif = false;
            let lb: gtk::ListBox = ui.get_object("content").unwrap();
            let ntasks_old = todo.len();
            while let Ok(tasks) = rx.try_recv() {

                let mut tasks = tasks.iter().cloned().collect::<Vec<_>>();
                tasks.sort_by(|a, b| a.uid.cmp(&b.uid));

                // This is incredibly nasty, but I'm not fucking able to loop
                // over the children of the listbox because it returns
                // Vec<Widget> instead of Vec<ListBoxRow>, and get_child is not
                // defined on Widget. So fuck you.
                for row in lb.get_children() {
                    row.destroy();
                    todo.clear();
                }

                // This is the ideal implementation that doesn't fucking work
//                for task in todo.difference(&tasks.clone()) {
//                    for row in lb.get_children() {
//                        let check = row.get_child().unwrap();
//                        let label = check.get_label().unwrap();
//                        if label == task.title {
//                            row.destroy();
//                        }
//                    }
//                }

                for task in tasks.iter() {
                    todo.insert(task.clone());
                    let check = CheckButton::new_with_label(&task.title);
                    lb.add(&check);
//                    notif = true;
                }

                lb.show_all();
            }

//            if notif {
            if ntasks_old != todo.len() {
                notify(todo.len());
            }
        }
    });
    glib::Continue(true)
}

fn connect(creds: Creds, tx: Sender<HashSet<Task>>, rx: Receiver<Message>) {
    loop {
        println!("Trying {}:{}... ", creds.host, creds.port);
        match get_connection(&creds) {
            Err(e) => {
                println!("  {:?}", e);
                std::thread::sleep(Duration::new(SLEEP, 0));
            },
            Ok(mut imap) => {
                println!("Connected!");
                poll_imap(&mut imap, &tx, &rx);
                break;
            },
        };
        std::thread::sleep(Duration::new(SLEEP, 0));
    }
}

fn poll_imap(
    mut imap: &mut IMAPStream,
    tx: &Sender<HashSet<Task>>,
    rx: &Receiver<Message>
) {
    loop {
        match get_tasks(&mut imap) {
            Ok(tasks) => { tx.send(tasks); },
            Err(e) => println!("Error getting tasks: {}", e),
        }

        if let Ok(Message::Quit) = rx.try_recv() {
            // Since we are exiting, no big deal if it fails
            let _ = imap.logout();
            break;
        }
        std::thread::sleep(Duration::new(SLEEP, 0));
    }
}

fn get_credentials() -> Result<Creds> {
    let mut path = try!(std::env::home_dir().ok_or("Can't get home dir"));

    // Build path to config file
    path.push(MUTT);
    path.push(CONF);

    let content = try!(read_config_file(path.as_path()));
    let user = try!(extract_info(r"set imap_user=(\w*)", &content));
    let pass = try!(extract_info(r"set imap_pass=(\w*)", &content));
    let host = try!(extract_info(r"set folder=imaps?://(.+):\d+", &content));
    let port = try!(extract_info(r"set folder=imaps?://.+:(\d+)", &content));
    let port = try!(port.parse());

    Ok(Creds {user: user, pass: pass, host: host, port: port})
}

fn read_config_file(path: &Path) -> Result<String> {
    let mut content = String::new();
    let mut file = try!(File::open(&path));
    try!(file.read_to_string(&mut content));
    Ok(content)
}

fn extract_info(pattern: &str, text: &str) -> Result<String> {
    let re = try!(Regex::new(pattern));
    let cap = try!(re.captures(text).ok_or("Couldn't match"));
    let xtr = try!(cap.at(1).ok_or("No captures"));
    Ok(xtr.to_string())
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
    let mbox = try!(imap.select(MBOX));
    for seqn in 1..mbox.exists+1 {
        let seq = &seqn.to_string();
        let uid = try!(get_uid(imap, seq));
        let subj = try!(get_subj(imap, seq));
        tasks.insert(Task {title: subj, uid: uid});
    }
    println!("{:?}", tasks);
    Ok(tasks)
}

fn get_uid(imap: &mut IMAPStream, seq: &str) -> Result<u64> {
    let resp = try!(imap.fetch(seq, "uid"));
    let uid = try!(extract_info(r".* FETCH \(UID (\d+)\)", &resp[0]));
    let uid = try!(uid.parse());
    Ok(uid)
}

fn get_subj(imap: &mut IMAPStream, seq: &str) -> Result<String> {
    let lines = try!(imap.fetch(seq, "body[header]"));

    let mut headers = String::new();
    for line in lines {
        headers = headers + &line;
    }

    let subj = try!(extract_info(r"Subject: (.*)\r", &headers));
    Ok(subj)
}

fn notify(tasks: usize) {
    println!("{:?} pending tasks", tasks);
    if let Err(e) = Notification::new()
        .summary(NAME)
        .body(&format!("{} tasks pending", tasks))
        .icon(ICON)
        .timeout(5000)
        .show() { println!("Couldn't show notification: {:?}", e) }
}
