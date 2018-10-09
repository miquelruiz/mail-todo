extern crate dirs;

use regex::Regex;

use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use {Creds, Result};

pub fn get_credentials(conf: String) -> Result<Creds> {
    let mut path = dirs::home_dir().ok_or("Can't get home dir")?;

    // Build path to config file
    path.push(conf);

    let content = read_config_file(path.as_path())?;
    let user = extract_info(r"set imap_user=(\w*)", &content)?;
    let pass = extract_info(r"set imap_pass=(\w*)", &content)?;
    let host = extract_info(r"set folder=imaps?://(.+):\d+", &content)?;
    let port = extract_info(r"set folder=imaps?://.+:(\d+)", &content)?;
    let port = port.parse()?;

    Ok(Creds {
        user: user,
        pass: pass,
        host: host,
        port: port,
    })
}

pub fn extract_info(pattern: &str, text: &str) -> Result<String> {
    let re = Regex::new(pattern)?;
    let cap = re.captures(text).ok_or("Couldn't match")?;
    let xtr = cap.get(1).ok_or("No captures")?;
    Ok(xtr.as_str().to_string())
}

fn read_config_file(path: &Path) -> Result<String> {
    let mut content = String::new();
    let mut file = File::open(&path)?;
    file.read_to_string(&mut content)?;
    Ok(content)
}

pub fn get_db_path() -> Result<String> {
    let mut path = dirs::home_dir().ok_or("Can't get home dir")?;
    path.push(::DB);
    let path_str = path.to_str()
        .ok_or("Can't convert path into string")?;
    Ok(path_str.to_string())
}
