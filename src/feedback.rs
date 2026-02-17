//! Audio feedback for recording state

use anyhow::{Context, Result};
use rodio::{Decoder, OutputStreamBuilder, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use tracing::info;

/// Audio feedback player
pub struct FeedbackPlayer {
    /// Whether sound feedback is enabled
    enabled: bool,
}

impl FeedbackPlayer {
    /// Create new feedback player
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Play sound file
    pub fn play(&self, path: &Path) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let file = File::open(path).context("Failed to open sound file")?;
        let source = Decoder::new(BufReader::new(file)).context("Failed to decode sound file")?;

        let stream =
            OutputStreamBuilder::open_default_stream().context("Failed to get audio output")?;
        let sink = Sink::connect_new(stream.mixer());

        sink.append(source);
        sink.sleep_until_end();

        info!("Played sound: {}", path.display());

        Ok(())
    }
}
