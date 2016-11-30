extern crate imap;
extern crate notify_rust;
extern crate openssl;
extern crate regex;

#[macro_use]
extern crate log;

pub mod notifier;
pub mod parser;
pub mod poller;

pub const CONF: &'static str = "miquelruiz.net";
pub const ICON: &'static str = "task-due";
pub const MBOX: &'static str = "ToDo";
pub const MUTT: &'static str = ".mutt";
pub const NAME: &'static str = "Mail-todo";
pub const NOTIF_TIMEOUT: i32 = 5000;
pub const SLEEP: u64 = 10;

pub type Result<T> = std::result::Result<T, Box<std::error::Error>>;

#[derive(Debug)]
pub enum Message {
    Awake,
    Connected,
    Delete(u64),
    NotConnected,
    Tasks(std::collections::HashSet<Task>),
    Quit,
}

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct Task {
    pub title: String,
    pub uid: u64,
}

#[derive(Debug)]
pub struct Creds {
    pub user: String,
    pub pass: String,
    pub host: String,
    pub port: u16,
}
