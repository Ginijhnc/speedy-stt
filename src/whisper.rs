//! Whisper model loading and inference

use anyhow::{Context, Result};
use std::path::Path;
use tracing::info;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Whisper transcription engine
pub struct WhisperEngine {
    /// Whisper context
    ctx: WhisperContext,
    /// Number of threads for inference
    threads: usize,
    /// Language code
    language: String,
}

impl WhisperEngine {
    /// Load Whisper model
    pub fn load(model_path: &Path, threads: usize, language: String) -> Result<Self> {
        info!("Loading Whisper model from: {}", model_path.display());

        let ctx = WhisperContext::new_with_params(
            model_path.to_str().context("Invalid model path")?,
            WhisperContextParameters::default(),
        )
        .context("Failed to load Whisper model")?;

        info!("Whisper model loaded successfully");

        Ok(Self {
            ctx,
            threads,
            language,
        })
    }

    /// Transcribe audio samples
    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(i32::try_from(self.threads).unwrap_or(4));
        params.set_language(Some(&self.language));
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_realtime(false);

        let mut state = self
            .ctx
            .create_state()
            .context("Failed to create Whisper state")?;
        state
            .full(params, samples)
            .context("Failed to transcribe audio")?;

        let num_segments = state.full_n_segments();
        let mut text = String::new();

        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                let segment_text = segment.to_str().context("Failed to get segment text")?;
                text.push_str(segment_text);
            }
        }

        Ok(text.trim().to_string())
    }
}
