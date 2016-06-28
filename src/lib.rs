extern crate notify_rust;
extern crate regex;

pub mod notifier;
pub mod parser;

pub const CONF: &'static str = "miquelruiz.net";
pub const ICON: &'static str = "task-due";
pub const MUTT: &'static str = ".mutt";
pub const NAME: &'static str = "Mail-todo";

pub type Result<T> = std::result::Result<T, Box<std::error::Error>>;
