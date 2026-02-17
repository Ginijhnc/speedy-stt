# Lazy Model Loading - Implementation Plan

## User prompt

I want you to read the .md files in this project, read some source code so you can get familiar with the codebase, and provide a thorough implementation plan with code snippets for the following feature: as of right now, the application consumes around 450MB of RAM even when idle. To my understanding this is because of the whisper model loaded in memory. So, I thought about "lazily" loading it. Basically as soon as the user presses the hotkeys to start recording, THEN you load the model. And after the transcription is done, you drop the model. I like this idea because I typically record 5-20 second audios, I rarely record a 1 or 2 second audio (in that specific case I will feel the lag/delay because of the model loading, but this is an acceptable tradeoff to me). Tell me what issues you see with this, edge cases, etc.

## Goal

Reduce idle RAM from ~450MB to ~15-25MB by loading the Whisper model on-demand (when the user presses the hotkey) and unloading it after a configurable cooldown period of inactivity.

## Strategy

- **Load trigger:** On hotkey press, start model loading in a background thread *in parallel* with audio recording. The user hears the start beep and begins recording immediately with zero perceived delay.
- **Unload trigger:** After transcription completes, start a cooldown timer (default 15 seconds). If the user presses the hotkey again within that window, reuse the loaded model. If the timer expires, drop the model to free memory.

## Files Changed

| File | Change |
|------|--------|
| `src/app.rs` | Main changes: new fields, parallel load logic, cooldown timer |
| `src/whisper.rs` | No changes needed |
| `src/config.rs` | Add `model_unload_delay_secs` field |
| `.env` | Add `MODEL_UNLOAD_DELAY_SECS=15` |
| `.env.example` | Add `MODEL_UNLOAD_DELAY_SECS=15` |

## Step 1: Add config field

Add `MODEL_UNLOAD_DELAY_SECS` to `.env`, `.env.example`, and `Config`.

**.env / .env.example** - append:
```env
# Seconds to keep the Whisper model in memory after transcription before unloading (0 = unload immediately)
MODEL_UNLOAD_DELAY_SECS=15
```

**src/config.rs** - add field to `Config`:
```rust
pub model_unload_delay_secs: u64,
```

And in `Config::load()`:
```rust
model_unload_delay_secs: Self::get_env("MODEL_UNLOAD_DELAY_SECS")?
    .parse()
    .context("Invalid MODEL_UNLOAD_DELAY_SECS")?,
```

## Step 2: Restructure `App` fields

Replace the eagerly loaded `whisper: WhisperEngine` with lazy loading state.

```rust
pub struct App {
    tray: TrayManager,
    hotkey: HotkeyListener,
    feedback: FeedbackPlayer,
    injector: TextInjector,
    volume_boost: f32,

    // Lazy model loading state
    /// Loaded whisper engine, or None if unloaded
    whisper: Option<WhisperEngine>,
    /// Background thread handle for in-progress model loading
    model_load_handle: Option<JoinHandle<Result<WhisperEngine>>>,
    /// When the model was last used (for cooldown-based unloading)
    last_model_use: Option<Instant>,

    // Config values needed for lazy loading
    model_path: PathBuf,
    whisper_threads: usize,
    whisper_language: String,
    model_unload_delay: Duration,
}
```

## Step 3: Update `App::new()`

Remove the `WhisperEngine::load()` call. Store config values instead.

```rust
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
```

## Step 4: Add cooldown check in the event loop

In `App::run()`, inside the main `loop`, after pumping messages and checking quit, add a cooldown check that runs every iteration (every 10ms, cheap check):

```rust
// Unload model if cooldown expired
if self.whisper.is_some() && !is_recording {
    if let Some(last_use) = self.last_model_use {
        if last_use.elapsed() >= self.model_unload_delay {
            self.whisper = None;
            self.last_model_use = None;
            info!("Whisper model unloaded after cooldown");
        }
    }
}
```

## Step 5: Trigger parallel model load in `start_recording()`

When the hotkey is pressed, start recording immediately. If the model is not loaded and not already loading, spawn a background thread to load it:

```rust
fn start_recording(
    &mut self,
    stop_signal: Arc<Mutex<bool>>,
) -> Result<JoinHandle<Result<Vec<f32>>>> {
    info!("Hotkey pressed - starting recording");
    self.tray.set_state(TrayState::Recording)?;

    if let Err(e) = self.feedback.play(&PathBuf::from("./assets/sounds/start.mp3")) {
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

    // Start recording immediately (zero delay for the user)
    *stop_signal.lock().unwrap() = false;
    let recorder = AudioRecorder::new(self.volume_boost);

    Ok(std::thread::spawn(move || {
        recorder.record_until_stopped(stop_signal)
    }))
}
```

## Step 6: Resolve model and transcribe in `finish_recording()`

When the hotkey is released, stop recording. If the model is still loading, wait for it. Then transcribe. Set `last_model_use` instead of dropping the model immediately.

```rust
fn finish_recording(
    &mut self,
    stop_signal: &Arc<Mutex<bool>>,
    recording_thread: &mut Option<JoinHandle<Result<Vec<f32>>>>,
) -> Result<()> {
    info!("Hotkey released - stopping recording");
    *stop_signal.lock().unwrap() = true;

    // Resolve the model: wait for background load if needed
    if self.whisper.is_none() {
        if let Some(handle) = self.model_load_handle.take() {
            match handle.join() {
                Ok(Ok(engine)) => {
                    info!("Whisper model loaded successfully");
                    self.whisper = Some(engine);
                }
                Ok(Err(e)) => {
                    error!("Failed to load Whisper model: {}", e);
                    self.tray.set_state(TrayState::Idle)?;
                    // Discard the recording since we can't transcribe
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
    }

    if let Some(thread) = recording_thread.take() {
        match thread.join() {
            Ok(Ok(samples)) => {
                if let Err(e) = self.feedback.play(&PathBuf::from("./assets/sounds/finish.mp3")) {
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

                // Start cooldown timer instead of dropping immediately
                self.last_model_use = Some(Instant::now());
            }
            Ok(Err(e)) => {
                error!("Recording failed: {}", e);
                self.tray.set_state(TrayState::Idle)?;
                // Still keep the model loaded (it was loaded successfully),
                // start cooldown so it gets cleaned up
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
```

## Step 7: Add `Instant` import

In `app.rs`, add:
```rust
use std::time::{Duration, Instant};
```

## Edge Cases Handled

| Edge Case | Handling |
|-----------|----------|
| Model loading fails mid-session | Log error, return to idle, discard recorded audio. App stays alive. |
| User releases hotkey before model finishes loading | `finish_recording` calls `handle.join()` and blocks until model is ready, then transcribes. Brief pause is expected. |
| Rapid press-release-press (double tap) | Model stays loaded during 15s cooldown window, so second press reuses it instantly. |
| User quits while model is loading | `App` is dropped, `JoinHandle` is dropped (thread is detached). Process exits cleanly. |
| `MODEL_UNLOAD_DELAY_SECS=0` | Model is unloaded on the next event loop tick (~10ms after transcription). Effectively immediate unload. |
| Model file deleted/moved between uses | Next load attempt fails gracefully, error is logged, app stays alive. |
| CPU contention during parallel load + record | Model loading is mostly I/O + memory allocation. Audio capture uses kernel ring buffer. No meaningful contention. |

## Memory Profile

| State | Before | After |
|-------|--------|-------|
| Idle (no recent use) | ~450MB | ~15-25MB |
| Idle (within cooldown) | ~450MB | ~450MB |
| Recording + loading | ~450MB+ | ~450MB (growing) |
| Transcribing | ~450MB+ | ~450MB+ |
| Post-transcription (cooldown) | ~450MB | ~450MB |
| Post-cooldown | ~450MB | ~15-25MB |
