extern crate gtk;
use gtk::prelude::*;
use gtk::{Builder, CheckButton, ListBox, Statusbar, StatusIcon, Window};

extern crate glib;

extern crate mail_todo;
use mail_todo::{Message, notifier, parser, poller, Task};

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver};
use std::thread;

thread_local!(
    static GLOBAL: RefCell<
        Option<(Builder, Receiver<Message>, HashMap<Task, bool>)>
    > = RefCell::new(None)
);

fn main() {
    if gtk::init().is_err() {
        panic!("Failed to initialize GTK");
    }

    let (stoptx, stoprx) = channel::<Message>();
    let (todotx, todorx) = channel::<Message>();

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
            poller::connect(creds, todotx, stoprx);
        }).unwrap();

    gtk::main();
    let _ = child.join();
}

fn receive() -> glib::Continue {
    GLOBAL.with(|global| {
        if let Some((ref ui, ref rx, ref mut todo)) = *global.borrow_mut() {
            while let Ok(msg) = rx.try_recv() { match msg {
                Message::Tasks(ref tasks) => update_list(ui, tasks, todo),
                Message::Status(st) => update_status(ui, st),
                Message::Quit => panic!("Main thread got a Quit message!"),
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
