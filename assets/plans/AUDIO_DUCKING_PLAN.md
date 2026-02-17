# Audio Ducking Implementation Plan

## User prompt

I want you to read the .md files in this project, pay close attention to the AGENTS.md so you can properly understand the code style i'm going for. Then, read some of the existing source code to get familiar with the codebase, and provide a thorough implementation plan with code snippets for the following feature: when i press the hotkeys to start recording, the background audio of my PC should be lowered to 0 and then resumed once stop pressing the hotkeys. Though it shouldn't be instantaneous... there should be a 500ms fade-out and fade-in of the PC audio. The goal here is to prevent my music from interfering with my recorded speech. Tell me what issues you see with this, edge cases, etc.

## Overview

A new module `volume.rs` will manage ducking (fading out) all other applications'
audio when recording starts, and restoring them when recording stops, using the
Windows Audio Session API (WASAPI). Ducking is always enabled with a hardcoded
500ms fade duration.

## New Module: `src/volume.rs`

### COM Call Chain

```
CoCreateInstance(&MMDeviceEnumerator) -> IMMDeviceEnumerator
  .GetDefaultAudioEndpoint(eRender, eMultimedia) -> IMMDevice
  .Activate::<IAudioSessionManager2>() -> IAudioSessionManager2
  .GetSessionEnumerator() -> IAudioSessionEnumerator
  .GetSession(i) -> IAudioSessionControl
  .cast::<IAudioSessionControl2>()  // filter by PID + system sounds
  .cast::<ISimpleAudioVolume>()     // get/set volume
```

### Structs

```rust
/// Stored state of a single ducked session.
struct DuckedSession {
    volume_control: ISimpleAudioVolume,
    original_volume: f32,
}

/// Manages fading other applications' audio during recording.
pub struct VolumeDucker {
    sessions: Vec<DuckedSession>,
}
```

### Constants

```rust
/// Duration of the fade-out and fade-in transitions.
const FADE_DURATION: Duration = Duration::from_millis(500);

/// Interval between volume steps during a fade.
const FADE_STEP_INTERVAL: Duration = Duration::from_millis(10);
```

### Public API

```rust
impl VolumeDucker {
    /// Enumerate all other audio sessions and fade them to silence.
    pub fn duck() -> Result<Self> { ... }

    /// Restore all ducked sessions to their original volume.
    pub fn restore(&self) -> Result<()> { ... }
}
```

### Session Filtering

The enumerator skips:

1. **Own process** -- `IAudioSessionControl2::GetProcessId()` vs `std::process::id()`
2. **System sounds** -- `IAudioSessionControl2::IsSystemSoundsSession()` returns
   `S_OK` (HRESULT value 0) for system sounds sessions
3. **Dead sessions** -- `GetProcessId()` returns 0 for expired sessions

### Fade Implementation

Step every 10ms for smoothness (50 steps over 500ms). Per-session errors during
fade are logged and ignored (the session's app may have exited).

```rust
fn fade(&self, direction: FadeDirection) -> Result<()> {
    let steps = (FADE_DURATION.as_millis() / FADE_STEP_INTERVAL.as_millis()).max(1);
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        for session in &self.sessions {
            let vol = match direction {
                FadeDirection::Out => session.original_volume * (1.0 - t),
                FadeDirection::In => session.original_volume * t,
            };
            // Ignore errors from sessions whose app may have exited
            let _ = unsafe { session.volume_control.SetMasterVolume(vol, None) };
        }
        std::thread::sleep(FADE_STEP_INTERVAL);
    }
    Ok(())
}
```

### Drop Implementation

`VolumeDucker` implements `Drop` to restore volumes on panic or early return.
This does not protect against `taskkill /f` or power loss, which is an inherent
limitation shared by Windows' own communications ducking feature.

```rust
impl Drop for VolumeDucker {
    fn drop(&mut self) {
        // Best-effort restore on abnormal exit
        let _ = self.restore();
    }
}
```

## Integration into `App` (`src/app.rs`)

### New Field

```rust
pub struct App {
    // ... existing fields ...
    volume_ducker: Option<VolumeDucker>,
}
```

### Recording Flow

**`start_recording()`** -- play start sound first, then duck:

```rust
// 1. Play start feedback sound (at full volume)
self.feedback.play(&PathBuf::from("./assets/sounds/start.mp3"));

// 2. Duck other apps
match VolumeDucker::duck() {
    Ok(ducker) => self.volume_ducker = Some(ducker),
    Err(e) => error!("Failed to duck audio: {}", e),
}
```

**`finish_recording()`** -- restore first, then play stop sound:

```rust
// 1. Restore other apps' volume
if let Some(ducker) = self.volume_ducker.take() {
    if let Err(e) = ducker.restore() {
        error!("Failed to restore audio: {}", e);
    }
}

// 2. Play stop feedback sound (at full volume)
self.feedback.play(&PathBuf::from("./assets/sounds/finish.mp3"));
```

Note: `take()` moves the ducker out of the Option, preventing `Drop` from
running a second restore. Subsequent `duck()` calls re-enumerate sessions fresh.

## Cargo.toml Changes

May need to add `Win32_System_Com_StructuredStorage` to the windows crate features
if `Activate` requires `PROPVARIANT`. Verify at build time.

## Files to Change

| File | Change |
|---|---|
| `src/volume.rs` | **New** -- `VolumeDucker` with `duck()`, `restore()`, and `Drop` |
| `src/main.rs` | Add `mod volume;` |
| `src/app.rs` | Add `volume_ducker` field, call duck/restore in recording flow |
| `Cargo.toml` | Possibly add `Win32_System_Com_StructuredStorage` feature |

## Known Limitations

- **500ms blocks the main thread**: acceptable for a push-to-talk app.
- **Apps starting after duck**: not ducked; extremely unlikely during recording.
- **Crash without restore**: other apps stay at volume 0 until manually fixed.
  `Drop` impl covers panics but not forced kills.
- **User adjusts volume while ducked**: restore overwrites with stale original.
  Same behavior as Windows' built-in ducking.
- **Non-default audio endpoints**: only the default render endpoint is ducked.

## Implementation Post-Mortem: Debugging "Zero Sessions Found"

**Observed symptom**: the user reported that audio ducking had no effect —
Chrome playing YouTube continued at full volume while recording. The
feature compiled and ran without errors, but nothing was being ducked.

Diagnostic `info!` logging was added throughout the WASAPI enumeration path to
trace exactly which sessions were found and why each was accepted or skipped.
Each attempt below describes what the log revealed and what was tried.

### Attempt 1: `SetMasterVolume` signature mismatch (compile error)

**Symptom**: the project did not build at all.

`SetMasterVolume` in the `windows` crate takes `*const GUID` for the event
context, not `Option<_>`. Passing `None` caused a type error. Fixed by passing
`std::ptr::null()`.

### Attempt 2: `cast()` method not in scope (compile error)

**Symptom**: two further compile errors after fixing Attempt 1.

`.cast::<T>()` on COM interfaces requires the `Interface` trait to be in scope.
The `windows` crate re-exports it as `windows::core::Interface`. Adding that
import resolved both errors.

### Attempt 3: Hypothesis — `CoInitializeEx` failing silently

**Symptom**: the app ran but Chrome was still not ducked. No error was logged.

The first hypothesis was that `CoInitializeEx` might be returning
`RPC_E_CHANGED_MODE` (thread already has COM in a different apartment), causing
`duck()` to bail out before enumerating any sessions. Logging the HRESULT
showed it returned `S_FALSE` (0x1) — already initialized in the same apartment,
which is fine. This was a dead end; COM initialization was not the problem.

As a correctness improvement, the code was updated to handle `RPC_E_CHANGED_MODE`
gracefully (proceed without re-initializing, and skip `CoUninitialize` on drop)
even though it wasn't the active failure mode.

### Attempt 4: Hypothesis — Chrome not on the default render endpoint

**Symptom**: the log showed `WASAPI session count: 3` — own process, system
sounds, and one session with PID=0. Chrome was completely absent.

Hypothesis: Chrome outputs to a non-default audio device, so enumerating only
the default endpoint misses it. Fix: replaced `GetDefaultAudioEndpoint` with
`EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)` to collect sessions from all
active render endpoints. The log then showed 3 endpoints with 5 total sessions —
but still 0 ducked. Chrome's session appeared (no longer absent) but was still
being filtered out at a later step.

### Attempt 5: Hypothesis — PID=0 wrongly treated as a dead session

**Symptom**: even after finding sessions on all endpoints, the log reported
0 sessions ducked. The sessions that previously appeared as "dead (PID=0)" were
now active by state check but still being skipped by the PID=0 dead-session
filter.

Chrome's audio renderer runs in a sandboxed subprocess. Windows prevents us from
opening that process, so `GetProcessId()` returns 0 even for a fully active
session. The PID=0 filter was discarding Chrome's session as if it were dead.

Fix: replaced the PID=0 heuristic with a proper state check using
`IAudioSessionControl::GetState()` and `AudioSessionStateExpired`. Sessions with
PID=0 that are not expired are now kept. After this fix the log showed Chrome's
session advancing past the PID check — but it was then immediately dropped at
the `IsSystemSoundsSession` filter. Still 0 ducked.

### Attempt 6 (root cause): `IsSystemSoundsSession` return value misread

**Symptom**: every session across every endpoint was being classified as a
system sounds session and skipped, including Chrome.

The plan document correctly states that `IsSystemSoundsSession` returns `S_OK`
(HRESULT 0) for system sounds and a different value for regular sessions.
Despite that, the implementation used `.is_ok()` to test the return value.

The `windows` crate's `.is_ok()` returns `true` for **any** non-error HRESULT,
including `S_FALSE` (value 1). `S_FALSE` is exactly what `IsSystemSoundsSession`
returns for a regular (non-system-sounds) session — meaning "no". Because
`.is_ok()` matched `S_FALSE` as success, every session was classified as system
sounds and skipped. The bug existed from the very first version and masked all
other issues.

Fix: compare the raw HRESULT to `HRESULT(0)` directly instead of using
`.is_ok()`:

```rust
// Wrong: matches S_FALSE (1) too, skipping every session
if unsafe { control2.IsSystemSoundsSession() }.is_ok() { ... }

// Correct: matches only S_OK (0), the genuine "yes this is system sounds" value
if unsafe { control2.IsSystemSoundsSession() } == HRESULT(0) { ... }
```

After this fix Chrome's sessions were found, ducked, and restored correctly.
The user confirmed the feature worked.