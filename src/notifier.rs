use notify_rust::Notification;

pub fn notify(tasks: usize) {
    println!("{:?} pending tasks", tasks);
    if let Err(e) = Notification::new()
        .summary(::NAME)
        .body(&format!("{} tasks pending", tasks))
        .icon(::ICON)
        .timeout(5000)
        .show() { println!("Couldn't show notification: {:?}", e) }
}
