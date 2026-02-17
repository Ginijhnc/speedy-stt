//! Speedy-STT: push-to-talk speech-to-text dictation for Windows.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod audio;
mod config;
mod feedback;
mod hotkey;
mod input;
mod tray;
mod volume;
mod whisper;

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;

use app::App;
use config::Config;

/// Main entry point: load configuration, set up logging, and run the app.
fn main() -> Result<()> {
    let config = Config::load().context("Failed to load configuration")?;
    setup_logging(&config)?;
    App::new(config)?.run()
}

/// Configure tracing based on the log level and output destination in config.
fn setup_logging(config: &Config) -> Result<()> {
    let level = match config.log_level.as_str() {
        "trace" => "trace",
        "debug" => "debug",
        "warn" => "warn",
        "error" => "error",
        _ => "info",
    };
    // enigo is suppressed to error-only to prevent transcribed text from leaking into the log file
    let filter = EnvFilter::new(format!("{level},enigo=error"));
    let subscriber = tracing_subscriber::fmt().with_env_filter(filter);

    if config.log_to_file {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("speedy-stt.log")
            .context("Failed to open log file")?;
        subscriber.with_writer(file).init();
    } else {
        subscriber.init();
    }

    Ok(())
}
