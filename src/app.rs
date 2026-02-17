//! Application state and main event loop.
//!
//! Owns all runtime components and drives the push-to-talk recording cycle,
//! delegating each concern to the appropriate module.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tracing::{error, info};

#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
};

use crate::audio::AudioRecorder;
use crate::config::Config;
use crate::feedback::FeedbackPlayer;
use crate::hotkey::HotkeyListener;
use crate::input::TextInjector;
use crate::tray::{TrayManager, TrayState};
use crate::whisper::WhisperEngine;

/// Holds all runtime components and drives the event loop.
pub struct App {
    /// System tray manager
    tray: TrayManager,
    /// Global hotkey listener
    hotkey: HotkeyListener,
    /// Audio feedback player
    feedback: FeedbackPlayer,
    /// Text injection into the active window
    injector: TextInjector,
    /// Volume boost applied to recorded audio
    volume_boost: f32,
    /// Loaded Whisper engine, or None if currently unloaded
    whisper: Option<WhisperEngine>,
    /// Background thread handle for in-progress model loading
    model_load_handle: Option<JoinHandle<Result<WhisperEngine>>>,
    /// Timestamp of the last completed transcription, used for cooldown-based unloading
    last_model_use: Option<Instant>,
    /// Path to the Whisper model file
    model_path: PathBuf,
    /// Number of CPU threads to use for Whisper inference
    whisper_threads: usize,
    /// Language code for transcription
    whisper_language: String,
    /// How long to keep the model loaded after the last use before unloading
    model_unload_delay: Duration,
}

impl App {
    /// Initialize all components from the provided configuration.
    pub fn new(config: Config) -> Result<Self> {
        let tray = TrayManager::new().context("Failed to create system tray")?;
        let hotkey = HotkeyListener::new(&config.hotkey_modifier, &config.hotkey_key)
            .context("Failed to create hotkey listener")?;
        let feedback = FeedbackPlayer::new(config.enable_sound_feedback);
        let injector = TextInjector::new();
        let model_path = PathBuf::from(format!("./assets/models/{}", config.whisper_model));

        info!(
            "Speedy-STT ready. Hold {} + {} to record.",
            config.hotkey_modifier, config.hotkey_key
        );

        Ok(Self {
            tray,
            hotkey,
            feedback,
            injector,
            volume_boost: config.volume_boost,
            whisper: None,
            model_load_handle: None,
            last_model_use: None,
            model_path,
            whisper_threads: config.whisper_threads,
            whisper_language: config.whisper_language,
            model_unload_delay: Duration::from_secs(config.model_unload_delay_secs),
        })
    }

    /// Run the event loop until the user requests quit.
    pub fn run(mut self) -> Result<()> {
        self.tray.set_state(TrayState::Idle)?;

        let receiver = global_hotkey::GlobalHotKeyEvent::receiver();
        let mut is_recording = false;
        let stop_signal = Arc::new(Mutex::new(false));
        let mut recording_thread: Option<JoinHandle<Result<Vec<f32>>>> = None;

        loop {
            Self::pump_messages();

            if self.tray.should_quit() {
                info!("Quit requested");
                break;
            }

            if let Ok(event) = receiver.try_recv()
                && event.id == self.hotkey.hotkey.id()
            {
                match event.state {
                    global_hotkey::HotKeyState::Pressed => {
                        if !is_recording {
                            is_recording = true;
                            recording_thread =
                                Some(self.start_recording(Arc::clone(&stop_signal))?);
                        }
                    }
                    global_hotkey::HotKeyState::Released => {
                        if is_recording {
                            is_recording = false;
                            self.finish_recording(&stop_signal, &mut recording_thread)?;
                        }
                    }
                }
            }

            // Unload model if the cooldown period has expired
            if self.whisper.is_some()
                && !is_recording
                && let Some(last_use) = self.last_model_use
                && last_use.elapsed() >= self.model_unload_delay
            {
                self.whisper = None;
                self.last_model_use = None;
                info!("Whisper model unloaded after cooldown");
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        Ok(())
    }

    /// Start recording audio in a background thread and trigger model loading in parallel.
    fn start_recording(
        &mut self,
        stop_signal: Arc<Mutex<bool>>,
    ) -> Result<JoinHandle<Result<Vec<f32>>>> {
        info!("Hotkey pressed - starting recording");

        self.tray.set_state(TrayState::Recording)?;

        if let Err(e) = self
            .feedback
            .play(&PathBuf::from("./assets/sounds/start.mp3"))
        {
            error!("Failed to play start sound: {}", e);
        }

        // Start model loading in parallel if not already loaded or loading
        if self.whisper.is_none() && self.model_load_handle.is_none() {
            let path = self.model_path.clone();
            let threads = self.whisper_threads;
            let language = self.whisper_language.clone();

            info!("Loading Whisper model in background...");
            self.model_load_handle = Some(std::thread::spawn(move || {
                WhisperEngine::load(&path, threads, language)
            }));
        }

        *stop_signal.lock().unwrap() = false;
        let recorder = AudioRecorder::new(self.volume_boost);

        Ok(std::thread::spawn(move || {
            recorder.record_until_stopped(stop_signal)
        }))
    }

    /// Stop recording, wait for the model if still loading, then transcribe and inject the result.
    fn finish_recording(
        &mut self,
        stop_signal: &Arc<Mutex<bool>>,
        recording_thread: &mut Option<JoinHandle<Result<Vec<f32>>>>,
    ) -> Result<()> {
        info!("Hotkey released - stopping recording");

        *stop_signal.lock().unwrap() = true;

        // Resolve the model: wait for background load if needed
        if self.whisper.is_none()
            && let Some(handle) = self.model_load_handle.take()
        {
            match handle.join() {
                Ok(Ok(engine)) => {
                    info!("Whisper model loaded successfully");
                    self.whisper = Some(engine);
                }
                Ok(Err(e)) => {
                    error!("Failed to load Whisper model: {}", e);
                    self.tray.set_state(TrayState::Idle)?;
                    if let Some(thread) = recording_thread.take() {
                        let _ = thread.join();
                    }
                    return Ok(());
                }
                Err(_) => {
                    error!("Model loading thread panicked");
                    self.tray.set_state(TrayState::Idle)?;
                    if let Some(thread) = recording_thread.take() {
                        let _ = thread.join();
                    }
                    return Ok(());
                }
            }
        }

        if let Some(thread) = recording_thread.take() {
            match thread.join() {
                Ok(Ok(samples)) => {
                    if let Err(e) = self
                        .feedback
                        .play(&PathBuf::from("./assets/sounds/finish.mp3"))
                    {
                        error!("Failed to play stop sound: {}", e);
                    }

                    self.tray.set_state(TrayState::Idle)?;
                    info!("Recording stopped, transcribing...");

                    if let Some(ref whisper) = self.whisper {
                        match whisper.transcribe(&samples) {
                            Ok(text) if !text.is_empty() => {
                                if let Err(e) = self.injector.inject(&text) {
                                    error!("Failed to inject text: {}", e);
                                }
                                info!("Transcription complete");
                            }
                            Ok(_) => info!("Transcription complete (empty result)"),
                            Err(e) => error!("Transcription failed: {}", e),
                        }
                    }

                    // Start cooldown timer instead of dropping the model immediately
                    self.last_model_use = Some(Instant::now());
                }
                Ok(Err(e)) => {
                    error!("Recording failed: {}", e);
                    self.tray.set_state(TrayState::Idle)?;
                    self.last_model_use = Some(Instant::now());
                }
                Err(_) => {
                    error!("Recording thread panicked");
                    self.tray.set_state(TrayState::Idle)?;
                    self.last_model_use = Some(Instant::now());
                }
            }
        }

        Ok(())
    }

    /// Pump the Windows message queue so tray and hotkey events are delivered.
    fn pump_messages() {
        #[cfg(windows)]
        // SAFETY: MSG is a plain Windows struct; PeekMessageW, TranslateMessage,
        // and DispatchMessageW are standard message-loop calls with no invariants
        // beyond what the Windows API guarantees.
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}
