extern crate gtk;
use gtk::prelude::*;
use gtk::{
    Builder, CheckButton, ListBox, ListBoxRow, Statusbar, StatusIcon, Window
};

extern crate glib;

#[macro_use]
extern crate log;
extern crate env_logger;

extern crate mail_todo;
use mail_todo::{Message, notifier, parser, poller, Task};

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

thread_local!(
    static GLOBAL: RefCell<
        Option<(Builder, Sender<Message>, Receiver<Message>)>
    > = RefCell::new(None)
);

fn main() {
    if gtk::init().is_err() {
        panic!("Failed to initialize GTK");
    }

    if let Err(e) = env_logger::init() {
        panic!("Couldn't initialize logger: {:?}", e);
    }

    let (imap_tx, imap_rx) = channel::<Message>();
    let (ui_tx, ui_rx)     = channel::<Message>();

    let ui = include_str!("../resources/ui.glade");
    let builder = Builder::new_from_string(ui);

    let stop = imap_tx.clone();
    let window: Window = builder.get_object("window").unwrap();
    window.connect_delete_event(move |_, _| {
        info!("Closing...");
        let _ = stop.send(Message::Quit).unwrap();
        gtk::main_quit();
        Inhibit(false)
    });

    let icon = StatusIcon::new_from_icon_name(mail_todo::ICON);
    icon.connect_activate(move |_| {
        window.set_visible(!window.is_visible());
    });

    let imap_tx2 = imap_tx.clone();
    GLOBAL.with(move |global| {
        *global.borrow_mut() = Some((builder, imap_tx2, ui_rx))
    });
    glib::timeout_add(100, receive);

    let creds = parser::get_credentials().unwrap();
    let child = thread::Builder::new()
        .name("poller".to_string())
        .spawn(move || {
            poller::connect(creds, ui_tx, imap_tx, imap_rx);
        }).unwrap();

    gtk::main();
    let _ = child.join();
}

fn receive() -> glib::Continue {
    GLOBAL.with(|global| {
        if let Some((ref ui, ref tx, ref rx)) = *global.borrow_mut() {
            while let Ok(msg) = rx.try_recv() { match msg {
                Message::Tasks(ref tasks) => update_list(ui, tasks, tx),
                Message::Status(st) => update_status(ui, st),
                m => panic!("Main thread got unexpected message! {:?}", m),
            }}
        }
    });
    glib::Continue(true)
}

fn update_list(
    ui: &Builder,
    tasks: &HashSet<Task>,
    tx: &Sender<Message>,
) {
    let lb: ListBox = ui.get_object("content").unwrap();
    let mut notify = false;

    // titles will serve to keep track of what's in the UI and what's not
    let mut titles: HashSet<&str> = HashSet::new();
    for t in tasks.iter() {
        titles.insert(&t.title);
    }

    for wrow in lb.get_children() {
        let row: ListBoxRow = wrow.downcast().unwrap();
        let wcheck = row.get_child().unwrap();
        let check: CheckButton = wcheck.downcast().unwrap();
        let label = check.get_label().unwrap();

        if !titles.contains::<str>(&label)  {
            // If the row is not in the titles, needs to be deleted
            notify = true;
            row.destroy();
        } else {
            // If the row is in the titles, delete from titles so it's not
            // added again
            titles.remove::<str>(&label);
        }
    }

    // loop over the tasks because the contain the uid's
    for task in tasks.iter() {
        // If the task is not in the titles, means we've already seen it in
        // the interface
        if !titles.contains::<str>(&task.title) {
            continue
        }

        notify = true;
        let check = CheckButton::new_with_label(&task.title);
        lb.add(&check);

        // copy here the uid so the closure does not reference the task
        let uid = task.uid;
        let tx = tx.clone();
        check.connect_toggled(move |_|
            if let Err(e) = tx.send(Message::Delete(uid)) {
                error!("Couldn't delete {}: {}", uid, e);
            }
        );
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

fn update_status(ui: &Builder, status: &'static str) {
    let bar: Statusbar = ui.get_object("status").unwrap();
    let ctx = bar.get_context_id("whatever?");
    let _ = bar.push(ctx, status);
}
