extern crate getopts;
use getopts::Options;

extern crate gtk;
use gtk::prelude::*;
use gtk::{Builder, Button, CheckButton, ListBox, ListBoxRow, StatusIcon,
          Statusbar, Window};

extern crate glib;

#[macro_use]
extern crate log;
extern crate env_logger;

extern crate mail_todo;
use mail_todo::{backup, notifier, parser, poller, Message, Task};

use std::cell::RefCell;
use std::collections::HashSet;
use std::env;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

thread_local!(
    static GLOBAL: RefCell<
        Option<(Builder, Sender<Message>, Receiver<Message>)>
    > = RefCell::new(None)
);

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    opts.reqopt(
        "c",
        "config",
        "Path to the config file",
        "CONFIG",
    );
    opts.optopt(
        "f",
        "folder",
        "IMAP folder to monitor",
        "FOLDER",
    );

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            println!("{}", f.to_string());
            return;
        }
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }
    let conf = matches.opt_str("c").unwrap();
    let folder = if matches.opt_present("f") {
        matches.opt_str("f").unwrap()
    } else {
        String::from(mail_todo::MBOX)
    };

    if let Err(e) = gtk::init() {
        panic!("Failed to initialize GTK: {:?}", e);
    }

    env_logger::init();

    let (backup_tx, backup_rx) = channel::<Message>();
    let (imap_tx, imap_rx) = channel::<Message>();
    let (ui_tx, ui_rx) = channel::<Message>();

    let ui = include_str!("../resources/ui.glade");
    let builder = Builder::new_from_string(ui);

    let stop_poller = imap_tx.clone();
    let stop_backup = backup_tx.clone();
    let window: Window = builder.get_object("window").unwrap();
    window.connect_delete_event(move |_, _| {
        info!("Closing...");
        let _ = stop_poller.send(Message::Quit).unwrap();
        let _ = stop_backup.send(Message::Quit).unwrap();
        gtk::main_quit();
        Inhibit(false)
    });

    let icon = StatusIcon::new_from_icon_name(mail_todo::ICON);
    icon.connect_activate(move |_| {
        window.set_visible(!window.is_visible());
    });

    let del: Button = builder.get_object("delete").unwrap();
    del.connect_clicked(|_| {
        destroy_checked();
    });

    let imap_tx2 = imap_tx.clone();
    GLOBAL.with(move |global| {
        *global.borrow_mut() = Some((builder, imap_tx2, ui_rx))
    });
    glib::timeout_add(100, receive);

    let creds = parser::get_credentials(conf).unwrap();
    let poller_thread = thread::Builder::new()
        .name("poller".to_string())
        .spawn(move || {
            poller::start(creds, &folder, ui_tx, imap_tx, imap_rx);
        })
        .unwrap();

    let backup_thread = thread::Builder::new()
        .name("backup".to_string())
        .spawn(move || {
            backup::start(backup_tx, backup_rx);
        })
        .unwrap();

    gtk::main();
    info!("Waiting for all threads to finish");
    let _ = poller_thread.join();
    let _ = backup_thread.join();
}

fn receive() -> glib::Continue {
    GLOBAL.with(|global| {
        if let Some((ref ui, ref tx, ref rx)) = *global.borrow_mut() {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    Message::Tasks(ref tasks) => update_list(ui, tasks, tx),
                    Message::Connected => update_status(ui, "Connected", true),
                    Message::NotConnected => {
                        update_status(ui, "Connecting...", false)
                    }
                    m => panic!("Main thread got unexpected message! {:?}", m),
                }
            }
        }
    });
    glib::Continue(true)
}

fn update_list(ui: &Builder, tasks: &HashSet<Task>, tx: &Sender<Message>) {
    let lb: ListBox = ui.get_object("content").unwrap();
    let mut notify = false;

    // "titles" will serve to keep track of what's in the UI and what's not
    let mut titles: HashSet<&str> = HashSet::new();
    for t in tasks.iter() {
        titles.insert(&t.title);
    }

    // loop over the UI rows to see what needs to be deleted, and delete it
    for wrow in lb.get_children() {
        let row: ListBoxRow = wrow.downcast().unwrap();
        let wcheck = row.get_child().unwrap();
        let check: CheckButton = wcheck.downcast().unwrap();
        let label = check.get_label().unwrap();

        if !titles.contains::<str>(&label) {
            // If the row is not in the titles, needs to be deleted
            notify = true;
            row.destroy();
        } else {
            // If the row is in the titles, delete from titles so it's not
            // added again
            titles.remove::<str>(&label);
        }
    }

    // add whatever task is missing to the interface
    for task in tasks.iter() {
        // If the task is not in "titles", means we've already seen it in
        // the interface
        if !titles.contains::<str>(&task.title) {
            continue;
        }

        notify = true;
        let check = CheckButton::new_with_label(&task.title);
        lb.add(&check);

        // copy here the uid so the closure does not reference the task
        let uid = task.uid;
        let tx = tx.clone();
        debug!("Storing uid {} in destroy closure", uid);
        check.connect_destroy(move |_| {
            if let Err(e) = tx.send(Message::Delete(uid)) {
                error!("Couldn't send delete message {}: {}", uid, e);
            }
        });
    }

    lb.show_all();

    if notify {
        notifier::notify(
            &format!("{} tasks pending", tasks.len()),
            mail_todo::ICON,
            mail_todo::NOTIF_TIMEOUT,
        );
    }
}

fn update_status(ui: &Builder, status: &'static str, enable_btn: bool) {
    ui.get_object("status")
        .and_then(|b: Statusbar| {
            Some(b.push(b.get_context_id("status"), status))
        });
    ui.get_object("delete")
        .and_then(|d: Button| Some(d.set_sensitive(enable_btn)));
}

fn destroy_checked() {
    GLOBAL.with(|global| {
        if let Some((ref ui, _, _)) = *global.borrow_mut() {
            let lb: ListBox = ui.get_object("content").unwrap();
            for wrow in lb.get_children() {
                let row: ListBoxRow = wrow.downcast().unwrap();
                let wcheck = row.get_child().unwrap();
                let check: CheckButton = wcheck.downcast().unwrap();
                let label = check.get_label().unwrap();
                info!("Considering '{}'", label);

                if check.get_active() {
                    info!("Destroying '{}'", label);
                    row.destroy();
                }
            }
        }
    });
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}
