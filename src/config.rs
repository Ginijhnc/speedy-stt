//! Configuration loading from .env file

use anyhow::{Context, Result};

/// Application configuration loaded from .env
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct Config {
    pub volume_boost: f32,
    pub whisper_model: String,
    pub whisper_language: String,
    pub whisper_threads: usize,
    pub hotkey_modifier: String,
    pub hotkey_key: String,
    pub enable_sound_feedback: bool,
    pub log_to_file: bool,
    pub log_level: String,
    pub model_unload_delay_secs: u64,
}

impl Config {
    /// Load configuration from .env file
    pub fn load() -> Result<Self> {
        dotenvy::dotenv().context(
            "Missing .env file. Copy .env.example to .env and fill in the required values",
        )?;

        Ok(Self {
            volume_boost: Self::get_env("VOLUME_BOOST")?
                .parse()
                .context("Invalid VOLUME_BOOST")?,
            whisper_model: Self::get_env("WHISPER_MODEL")?,
            whisper_language: Self::get_env("WHISPER_LANGUAGE")?,
            whisper_threads: Self::get_env("WHISPER_THREADS")?
                .parse()
                .context("Invalid WHISPER_THREADS")?,
            hotkey_modifier: Self::get_env("HOTKEY_MODIFIER")?,
            hotkey_key: Self::get_env("HOTKEY_KEY")?,
            enable_sound_feedback: Self::get_env("ENABLE_SOUND_FEEDBACK")?
                .parse()
                .context("Invalid ENABLE_SOUND_FEEDBACK")?,
            log_to_file: Self::get_env("LOG_TO_FILE")?
                .parse()
                .context("Invalid LOG_TO_FILE")?,
            log_level: Self::get_env("LOG_LEVEL")?,
            model_unload_delay_secs: Self::get_env("MODEL_UNLOAD_DELAY_SECS")?
                .parse()
                .context("Invalid MODEL_UNLOAD_DELAY_SECS")?,
        })
    }

    /// Get environment variable with context
    fn get_env(key: &str) -> Result<String> {
        std::env::var(key)
            .context(format!("Missing or invalid environment variable: {key}. See .env.example for required configuration"))
    }
}
