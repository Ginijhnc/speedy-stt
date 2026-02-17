//! Application state and main event loop.
//!
//! Owns all runtime components and drives the push-to-talk recording cycle,
//! delegating each concern to the appropriate module.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

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
    /// Whisper transcription engine
    whisper: WhisperEngine,
    /// Volume boost applied to recorded audio
    volume_boost: f32,
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
        let whisper =
            WhisperEngine::load(&model_path, config.whisper_threads, config.whisper_language)
                .context("Failed to load Whisper model")?;

        info!(
            "Speedy-STT ready. Hold {} + {} to record.",
            config.hotkey_modifier, config.hotkey_key
        );

        Ok(Self {
            tray,
            hotkey,
            feedback,
            injector,
            whisper,
            volume_boost: config.volume_boost,
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

            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        Ok(())
    }

    /// Start recording audio in a background thread.
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

        *stop_signal.lock().unwrap() = false;
        let recorder = AudioRecorder::new(self.volume_boost);

        Ok(std::thread::spawn(move || {
            recorder.record_until_stopped(stop_signal)
        }))
    }

    /// Stop recording, wait for the thread, then transcribe and inject the result.
    fn finish_recording(
        &mut self,
        stop_signal: &Arc<Mutex<bool>>,
        recording_thread: &mut Option<JoinHandle<Result<Vec<f32>>>>,
    ) -> Result<()> {
        info!("Hotkey released - stopping recording");

        *stop_signal.lock().unwrap() = true;

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

                    match self.whisper.transcribe(&samples) {
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
                Ok(Err(e)) => {
                    error!("Recording failed: {}", e);
                    self.tray.set_state(TrayState::Idle)?;
                }
                Err(_) => {
                    error!("Recording thread panicked");
                    self.tray.set_state(TrayState::Idle)?;
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
