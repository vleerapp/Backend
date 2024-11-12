use chrono::Local;
use kdam::{term, BarExt, Column, RichProgress, Spinner};
use std::fs::{File, OpenOptions};
use std::io::{stderr, IsTerminal, Result, Write};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

const LOG_FILE: &str = "app.log";

pub fn init_logging() {
    term::init(stderr().is_terminal());
    term::hide_cursor().unwrap_or_default();
}

pub fn log_with_table(message: &str, table_data: Vec<(&str, String)>) -> Result<()> {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let log_message = format!("[{}] {}", timestamp, message);

    let pb = RichProgress::new(
        kdam::tqdm!(total = 0),
        vec![
            Column::Spinner(Spinner::new(
                &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
                80.0,
                1.0,
            )),
            Column::Text(match message.split_whitespace().next() {
                Some("✅") => format!("\x1b[1;32m{}\x1b[0m", log_message.replace("✅ ", "")),
                Some("💥") => format!("\x1b[1;31m{}\x1b[0m", log_message.replace("💥 ", "")), 
                Some("🚫") => format!("\x1b[1;33m{}\x1b[0m", log_message.replace("🚫 ", "")),
                Some("📥") => format!("\x1b[1;34m{}\x1b[0m", log_message.replace("📥 ", "")),
                Some("ℹ️") => format!("\x1b[1;36m{}\x1b[0m", log_message.replace("ℹ️ ", "")),
                _ => format!("\x1b[1;37m{}\x1b[0m", log_message),
            }),
        ],
    );

    let pb_arc = Arc::new(Mutex::new(pb));
    let pb_clone = Arc::clone(&pb_arc);
    let (tx, rx) = mpsc::channel();

    let spinner_thread = thread::spawn(move || {
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let mut pb = pb_clone.lock().unwrap();
            pb.refresh().unwrap_or_default();
        }
    });

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(LOG_FILE) {
        writeln!(file, "{}", log_message).unwrap_or_else(|e| {
            eprintln!("Failed to write to log file: {}", e);
        });
        for (key, value) in &table_data {
            writeln!(file, "{:<10}: {}", key, value).unwrap_or_else(|e| {
                eprintln!("Failed to write table data to log file: {}", e);
            });
        }
    }

    let _ = tx.send(());
    let _ = spinner_thread.join();

    eprintln!("\n✔ {}", match message.split_whitespace().next() {
        Some("✅") => format!("\x1b[1;32m{}\x1b[0m", log_message.replace("✅ ", "")),
        Some("💥") => format!("\x1b[1;31m{}\x1b[0m", log_message.replace("💥 ", "")),
        Some("🚫") => format!("\x1b[1;33m{}\x1b[0m", log_message.replace("🚫 ", "")),
        Some("📥") => format!("\x1b[1;34m{}\x1b[0m", log_message.replace("📥 ", "")),
        Some("ℹ️") => format!("\x1b[1;36m{}\x1b[0m", log_message.replace("ℹ️ ", "")),
        _ => format!("\x1b[1;37m{}\x1b[0m", log_message),
    });

    if !table_data.is_empty() {
        let max_key_len = table_data.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        let max_val_len = table_data.iter().map(|(_, v)| v.len()).max().unwrap_or(0);

        let separator = format!(
            "\x1b[36m+-{}-+-{}-+\x1b[0m",
            "-".repeat(max_key_len),
            "-".repeat(max_val_len)
        );

        let table_str = table_data
            .iter()
            .map(|(key, value)| {
                format!(
                    "\x1b[36m|\x1b[0m \x1b[33m{:<width_k$}\x1b[0m \x1b[36m|\x1b[0m \x1b[37m{:<width_v$}\x1b[0m \x1b[36m|\x1b[0m",
                    key,
                    value,
                    width_k = max_key_len,
                    width_v = max_val_len
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        eprintln!("{}", separator);
        eprintln!("{}", table_str);
        eprintln!("{}", separator);
    }

    Ok(())
}

pub fn clear_log() {
    if let Ok(file) = File::create(LOG_FILE) {
        if let Err(e) = file.set_len(0) {
            eprintln!("Failed to clear log file: {}", e);
        }
    }
}
