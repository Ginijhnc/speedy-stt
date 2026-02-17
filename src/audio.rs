//! Audio capture with volume boost

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

/// Audio recorder that captures from default microphone
pub struct AudioRecorder {
    /// Volume boost multiplier
    volume_boost: f32,
}

impl AudioRecorder {
    /// Create new audio recorder
    pub fn new(volume_boost: f32) -> Self {
        Self { volume_boost }
    }

    /// Record audio until stopped
    pub fn record_until_stopped(&self, stop_signal: Arc<Mutex<bool>>) -> Result<Vec<f32>> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No input device available")?;

        info!(
            "Using input device: {}",
            device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_else(|_| "Unknown".to_string())
        );

        let config = device
            .default_input_config()
            .context("Failed to get default input config")?;

        info!("Input config: {:?}", config);

        let samples = Arc::new(Mutex::new(Vec::new()));
        let samples_clone = Arc::clone(&samples);
        let volume_boost = self.volume_boost;

        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut samples_lock = samples_clone.lock().unwrap_or_else(|e| e.into_inner());
                for &sample in data {
                    samples_lock.push(sample * volume_boost);
                }
            },
            |err| warn!("Audio stream error: {}", err),
            None,
        )?;

        stream.play()?;

        // Wait until stop signal is set
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let should_stop = *stop_signal.lock().unwrap_or_else(|e| e.into_inner());
            if should_stop {
                break;
            }
        }

        drop(stream);

        let recorded_samples = Arc::try_unwrap(samples)
            .unwrap_or_else(|_| panic!("Failed to unwrap samples"))
            .into_inner()
            .unwrap_or_else(|e| e.into_inner());

        info!("Recorded {} samples", recorded_samples.len());

        Ok(recorded_samples)
    }
}
