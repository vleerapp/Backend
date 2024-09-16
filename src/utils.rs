use chrono::Local;
use colored::*;
use std::fs::{File, OpenOptions};
use std::io::Write;

const LOG_FILE: &str = "app.log";

pub fn log(message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let log_message = format!("[{}] {}", timestamp, message);

    let colored_message = match message.split_whitespace().next() {
        Some("✅") => log_message.green(),
        Some("💥") => log_message.red(),
        Some("🚫") => log_message.yellow(),
        _ => log_message.normal(),
    };
    println!("{}", colored_message);

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE)
    {
        if let Err(e) = writeln!(file, "{}", log_message) {
            eprintln!("Failed to write to log file: {}", e);
        }
    } else {
        eprintln!("Failed to open log file");
    }
}

pub fn clear_log() {
    if let Ok(file) = File::create(LOG_FILE) {
        if let Err(e) = file.set_len(0) {
            eprintln!("Failed to clear log file: {}", e);
        }
    } else {
        eprintln!("Failed to open log file for clearing");
    }
}