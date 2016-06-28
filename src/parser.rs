use regex::Regex;

use ::Result;
use std::env;
use std::io::prelude::*;
use std::fs::File;
use std::path::Path;

#[derive(Debug)]
pub struct Creds {
    pub user: String,
    pub pass: String,
    pub host: String,
    pub port: u16,
}

pub fn get_credentials() -> Result<Creds> {
    let mut path = try!(env::home_dir().ok_or("Can't get home dir"));

    // Build path to config file
    path.push(::MUTT);
    path.push(::CONF);

    let content = try!(read_config_file(path.as_path()));
    let user = try!(extract_info(r"set imap_user=(\w*)", &content));
    let pass = try!(extract_info(r"set imap_pass=(\w*)", &content));
    let host = try!(extract_info(r"set folder=imaps?://(.+):\d+", &content));
    let port = try!(extract_info(r"set folder=imaps?://.+:(\d+)", &content));
    let port = try!(port.parse());

    Ok(Creds {user: user, pass: pass, host: host, port: port})
}

pub fn extract_info(pattern: &str, text: &str) -> Result<String> {
    let re = try!(Regex::new(pattern));
    let cap = try!(re.captures(text).ok_or("Couldn't match"));
    let xtr = try!(cap.at(1).ok_or("No captures"));
    Ok(xtr.to_string())
}

fn read_config_file(path: &Path) -> Result<String> {
    let mut content = String::new();
    let mut file = try!(File::open(&path));
    try!(file.read_to_string(&mut content));
    Ok(content)
}
