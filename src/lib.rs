extern crate imap;
extern crate notify_rust;
extern crate openssl;
extern crate regex;

pub mod notifier;
pub mod parser;
pub mod poller;

pub const CONF: &'static str = "miquelruiz.net";
pub const ICON: &'static str = "task-due";
pub const MBOX: &'static str = "ToDo";
pub const MUTT: &'static str = ".mutt";
pub const NAME: &'static str = "Mail-todo";
pub const SLEEP: u64 = 10;

pub type Result<T> = std::result::Result<T, Box<std::error::Error>>;

pub enum Message {
    Quit,
    Status(&'static str),
    Tasks(std::collections::HashSet<Task>),
}

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct Task {
    pub title: String,
    pub uid: u64,
}
