use notify_rust::Notification;

pub fn notify(body: &str, icon: &str, timeout: i32) {
    if let Err(e) = Notification::new()
        .summary(::NAME)
        .body(body)
        .icon(icon)
        .timeout(timeout)
        .show() { println!("Couldn't show notification: {:?}", e) }
}
