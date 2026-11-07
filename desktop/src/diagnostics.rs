//! Only fixed diagnostic labels/status codes may be written here. Never pass
//! credentials, tokens, response bodies, activity details or request URLs.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
};

pub fn record(stage: &str, detail: &str) {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let directory = std::path::PathBuf::from(home).join("Library/Logs/Whoosh Bridge");
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    let path = directory.join("connection.log");
    if fs::metadata(&path).is_ok_and(|m| m.len() > 1_048_576) {
        let _ = fs::rename(&path, directory.join("connection.previous.log"));
    }
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
    {
        let _ = writeln!(
            file,
            "{} {stage} {detail}",
            chrono::Local::now().to_rfc3339()
        );
    }
}
