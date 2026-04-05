/// Write an informational message to stderr (captured by journald).
pub fn info(msg: impl std::fmt::Display) {
    eprintln!("[INFO]  {msg}");
}

/// Write a warning to stderr.
pub fn warn(msg: impl std::fmt::Display) {
    eprintln!("[WARN]  {msg}");
}

/// Write an error to stderr.
pub fn error(msg: impl std::fmt::Display) {
    eprintln!("[ERROR] {msg}");
}
