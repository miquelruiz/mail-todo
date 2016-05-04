extern crate imap;
extern crate openssl;

use openssl::ssl::{SslContext, SslMethod};
use imap::client::{IMAPStream, IMAPMailbox};

fn main() {
    println!("This is mail-todo");
    let creds = get_credentials();
    let tasks = count_tasks(creds);
    println!("{} tasks pending for today", tasks);
}

fn get_credentials() -> (String, String) {
    ("user".to_string(), "password".to_string())
}

fn count_tasks(creds: (String, String)) -> u32 {
    let mut imap_socket = match IMAPStream::connect(
        "mail.miquelruiz.net",
        993,
        Some(SslContext::new(SslMethod::Sslv23).unwrap())
    ) {
        Ok(s)  => s,
        Err(e) => panic!("{}", e),
    };

    if let Err(e) = imap_socket.login(&creds.0, &creds.1) {
        panic!("Error: {}", e)
    };

    let mbox = match imap_socket.select("ToDo") {
        Ok(m)  => m,
        Err(e) => panic!("Error selecting INBOX: {}", e)
    };

    match imap_socket.fetch("1", "body[header]") {
        Ok(lines) => {
            for line in lines.iter() {
                print!("{}", line)
            }
        },
        Err(e) => panic!("Error fetching mail: {}", e),
    }

    if let Err(e) = imap_socket.logout() {
        println!("Error {}", e)
    };

    mbox.exists
}
