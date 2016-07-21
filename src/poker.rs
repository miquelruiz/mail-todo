extern crate sqlite3;
use sqlite3::{DatabaseConnection}

use ::notifier::notify;

use std::thread::sleep;
use std::time::Duration;

use time;
use time::Tm;

#[derive(Debug)]
struct Entry {
    id: i32,
    time: String,
    msg: String,
}

fn duration() -> Duration { Duration::new(::SLEEP, 0) }

pub fn start() {
    loop {
        match DatabaseConnection::in_memory() {
            Ok(conn) => runit(conn),
            Err(e)   => println!("Error connecting to DB: {}", e.to_string()),
        }
        sleep(duration());
    }
}

fn runit (conn: DatabaseConnection) {
    loop {
        let now = time::now();
        for time in &times {
            println!("Oh, hai! I'll poke you at {} :)", time);
            if matches(time, now) {
                println!("Match!");
                notify(
                    "Me cago en tu puta madre paco!",
                    "appointment",
                    ::NOTIF_TIMEOUT,
                );
            }
        }
        sleep(duration());
    }
}

fn matches(time_str: &str, time: Tm) -> bool {
    let t: Vec<&str> = time_str.split(':').collect();
    let hour: i32 = t[0].parse().unwrap();
    let min:  i32 = t[1].parse().unwrap();

    hour == time.tm_hour && min == time.tm_min
}
