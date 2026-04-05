pub fn info(message: impl AsRef<str>) {
    eprintln!("[INFO] {}", message.as_ref());
}

pub fn warn(message: impl AsRef<str>) {
    eprintln!("[WARN] {}", message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    eprintln!("[ERROR] {}", message.as_ref());
}
