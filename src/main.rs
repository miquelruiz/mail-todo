extern crate gtk;
use gtk::prelude::*;
use gtk::{Builder, CheckButton, ListBox, Statusbar, StatusIcon, Window};

extern crate glib;

extern crate imap;
use imap::client::IMAPStream;

extern crate mail_todo;
use mail_todo::{notifier, parser, Result};
use mail_todo::parser::Creds;

extern crate openssl;
use openssl::ssl::{SslContext, SslMethod};

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

const MBOX: &'static str = "ToDo";
const SLEEP: u64 = 10;


enum Message {
    Quit,
}

enum UIMessage {
    Tasks(HashSet<Task>),
    Status(&'static str),
}

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
struct Task {
    title: String,
    uid: u64,
}

thread_local!(
    static GLOBAL: RefCell<
        Option<(Builder, Receiver<UIMessage>, HashMap<Task, bool>)>
    > = RefCell::new(None)
);

fn main() {
    if gtk::init().is_err() {
        panic!("Failed to initialize GTK");
    }

    let (stoptx, stoprx) = channel::<Message>();
    let (todotx, todorx) = channel::<UIMessage>();

    let ui = include_str!("../resources/ui.glade");
    let builder = Builder::new_from_string(ui);

    let window: Window = builder.get_object("window").unwrap();
    window.connect_delete_event(move |_, _| {
        println!("Closing...");
        let _ = stoptx.send(Message::Quit).unwrap();
        gtk::main_quit();
        Inhibit(false)
    });

    let icon = StatusIcon::new_from_icon_name(mail_todo::ICON);
    icon.connect_activate(move |_| {
        window.set_visible(!window.is_visible());
    });

    let todo: HashMap<Task, bool> = HashMap::new();

    GLOBAL.with(move |global| {
        *global.borrow_mut() = Some((builder, todorx, todo))
    });
    glib::timeout_add(100, receive);

    let creds = parser::get_credentials().unwrap();
    let child = thread::Builder::new()
        .name("poller".to_string())
        .spawn(move || {
            connect(creds, todotx, stoprx);
        }).unwrap();

    gtk::main();
    let _ = child.join();
}

fn receive() -> glib::Continue {
    GLOBAL.with(|global| {
        if let Some((ref ui, ref rx, ref mut todo)) = *global.borrow_mut() {
            while let Ok(msg) = rx.try_recv() { match msg {
                UIMessage::Tasks(ref tasks) => update_list(ui, tasks, todo),
                UIMessage::Status(st) => update_status(ui, st),
            }}
        }
    });
    glib::Continue(true)
}

fn update_list(
    ui: &Builder,
    tasks: &HashSet<Task>,
    todo: &mut HashMap<Task, bool>,
) {
    let lb: ListBox = ui.get_object("content").unwrap();
    let mut tasks = tasks.iter().cloned().collect::<Vec<_>>();
    let ntasks = todo.len();
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
    //for task in todo.difference(&tasks.clone()) {
    //    for row in lb.get_children() {
    //        let check = row.get_child().unwrap();
    //        let label = check.get_label().unwrap();
    //        if label == task.title {
    //            row.destroy();
    //        }
    //    }
    //}

    for task in tasks.iter() {
        todo.insert(task.clone(), true);
        let check = CheckButton::new_with_label(&task.title);
        lb.add(&check);
        check.connect_toggled(|c| delete_task(c.get_label().unwrap()));
    }

    lb.show_all();

    if ntasks != todo.len() {
        notifier::notify(todo.len());
    }
}

fn update_status(ui: &Builder, status: &'static str) {
    let bar: Statusbar = ui.get_object("status").unwrap();
    let ctx = bar.get_context_id("whatever?");
    let _ = bar.push(ctx, status);
}

fn delete_task(task: String) {
    println!("Should delete '{}'", task);
}

fn connect(creds: Creds, tx: Sender<UIMessage>, rx: Receiver<Message>) {
    loop {
        println!("Trying {}:{}... ", creds.host, creds.port);
        tx.send(UIMessage::Status("Connecting..."));
        match get_connection(&creds) {
            Err(e) => {
                println!("  {:?}", e);
                std::thread::sleep(Duration::new(SLEEP, 0));
            },
            Ok(mut imap) => {
                tx.send(UIMessage::Status("Connected"));
                poll_imap(&mut imap, &tx, &rx);
                break;
            },
        };
        std::thread::sleep(Duration::new(SLEEP, 0));
    }
}

fn poll_imap(
    mut imap: &mut IMAPStream,
    tx: &Sender<UIMessage>,
    rx: &Receiver<Message>
) {
    loop {
        match get_tasks(&mut imap) {
            Ok(tasks) => { tx.send(UIMessage::Tasks(tasks)); },
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
