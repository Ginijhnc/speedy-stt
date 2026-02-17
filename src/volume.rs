//! Audio ducking via the Windows Audio Session API (WASAPI).
//!
//! Enumerates all active audio sessions on the default render endpoint,
//! fades them to silence when recording starts, and restores them when
//! recording stops. A 500ms linear fade is applied in both directions.

use std::time::Duration;

use anyhow::{Context, Result};
use tracing::{debug, error, info, warn};

#[cfg(windows)]
use windows::Win32::Media::Audio::{
    AudioSessionStateExpired, DEVICE_STATE_ACTIVE, IAudioSessionControl, IAudioSessionControl2,
    IAudioSessionEnumerator, IAudioSessionManager2, IMMDeviceCollection, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator, eRender,
};
#[cfg(windows)]
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
#[cfg(windows)]
use windows::core::HRESULT;
#[cfg(windows)]
use windows::core::Interface;

/// Duration of the fade-out and fade-in transitions.
const FADE_DURATION: Duration = Duration::from_millis(500);

/// Interval between volume steps during a fade.
const FADE_STEP_INTERVAL: Duration = Duration::from_millis(10);

/// Direction of a volume fade.
#[non_exhaustive]
enum FadeDirection {
    /// Fade volume from original level down to silence.
    Out,
    /// Fade volume from silence back to the original level.
    In,
}

/// Stored state of a single ducked audio session.
struct DuckedSession {
    /// COM interface used to get and set the session's master volume.
    #[cfg(windows)]
    volume_control: ISimpleAudioVolume,
    /// Volume level recorded before ducking began.
    original_volume: f32,
}

/// `RPC_E_CHANGED_MODE`: COM is already initialized on this thread with a
/// different apartment model. The thread is usable; we must not uninitialize.
#[cfg(windows)]
const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x80010106_u32 as i32);

/// Manages fading other applications' audio during recording.
///
/// Created via [`VolumeDucker::duck`], which enumerates all active sessions
/// and fades them out. Restoring is done with [`VolumeDucker::restore`].
/// Implements [`Drop`] for best-effort restore on panic or early return.
pub struct VolumeDucker {
    /// Sessions that were ducked and need to be restored.
    sessions: Vec<DuckedSession>,
    /// Whether this instance initialized COM and must call CoUninitialize on drop.
    #[cfg(windows)]
    com_initialized: bool,
}

impl VolumeDucker {
    /// Enumerate all other audio sessions and fade them to silence.
    ///
    /// Returns a `VolumeDucker` that holds the original volumes for restoration.
    /// Skips the current process, system sound sessions, and dead sessions.
    #[cfg(windows)]
    pub fn duck() -> Result<Self> {
        // SAFETY: CoInitializeEx initializes COM for this thread. S_OK means we
        // initialized it fresh; S_FALSE means already initialized with the same
        // apartment (both require a matching CoUninitialize). RPC_E_CHANGED_MODE
        // means the thread already has COM in a different apartment — we can still
        // use COM but must not call CoUninitialize since we did not initialize it.
        let com_hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        info!("CoInitializeEx HRESULT: {:?}", com_hr);
        let com_initialized = if com_hr == RPC_E_CHANGED_MODE {
            info!("COM already initialized with different apartment; proceeding without re-init");
            false
        } else {
            com_hr.ok().context("Failed to initialize COM")?;
            true
        };

        let sessions = collect_sessions()?;
        info!("Audio ducking: found {} session(s) to duck", sessions.len());

        let ducker = Self {
            sessions,
            com_initialized,
        };
        ducker.fade(FadeDirection::Out)?;
        Ok(ducker)
    }

    /// Restore all ducked sessions to their original volume.
    ///
    /// Per-session errors are logged but do not abort the restore pass.
    #[cfg(windows)]
    pub fn restore(&self) -> Result<()> {
        self.fade(FadeDirection::In)
    }

    /// Apply a linear fade in the given direction across all ducked sessions.
    ///
    /// Per-session volume errors are logged and skipped; the session's app
    /// may have exited during recording.
    #[cfg(windows)]
    fn fade(&self, direction: FadeDirection) -> Result<()> {
        let steps = (FADE_DURATION.as_millis() / FADE_STEP_INTERVAL.as_millis()).max(1);

        #[allow(
            clippy::as_conversions,
            reason = "controlled cast within known range for linear interpolation"
        )]
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            for session in &self.sessions {
                let vol = match direction {
                    FadeDirection::Out => session.original_volume * (1.0 - t),
                    FadeDirection::In => session.original_volume * t,
                };
                // SAFETY: SetMasterVolume is a straightforward COM setter. We
                // pass a valid f32 in [0.0, 1.0] and a null event context (no
                // notification needed). Errors mean the session's app exited.
                match unsafe {
                    session
                        .volume_control
                        .SetMasterVolume(vol, std::ptr::null())
                } {
                    Ok(()) => debug!("SetMasterVolume({:.3}) ok", vol),
                    Err(e) => warn!("SetMasterVolume({:.3}) failed: {:?}", vol, e),
                }
            }
            std::thread::sleep(FADE_STEP_INTERVAL);
        }

        Ok(())
    }
}

#[cfg(windows)]
impl Drop for VolumeDucker {
    /// Best-effort restore on abnormal exit (e.g. panic).
    ///
    /// Does not protect against forced process termination.
    fn drop(&mut self) {
        if let Err(e) = self.restore() {
            error!("Failed to restore audio volumes on drop: {}", e);
        }
        // SAFETY: Balances the CoInitializeEx call in duck(), but only when we
        // actually initialized COM (not when RPC_E_CHANGED_MODE was returned).
        if self.com_initialized {
            unsafe { CoUninitialize() };
        }
    }
}

/// Enumerate audio sessions across all active render endpoints.
///
/// Returns a list of [`DuckedSession`] values ready to be faded, one per
/// active session that passes the filter in [`try_duck_session`].
#[cfg(windows)]
fn collect_sessions() -> Result<Vec<DuckedSession>> {
    // SAFETY: CoCreateInstance requires COM to be initialized (done in duck()).
    // MMDeviceEnumerator is a well-known CLSID with no additional invariants.
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .context("Failed to create IMMDeviceEnumerator")?;

    // Enumerate all active render endpoints so we catch apps (e.g. Chrome) that
    // output to a non-default device.
    // SAFETY: EnumAudioEndpoints is a standard COM query with no extra invariants.
    let devices: IMMDeviceCollection =
        unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) }
            .context("Failed to enumerate audio endpoints")?;

    // SAFETY: GetCount is a simple getter with no invariants.
    let device_count = unsafe { devices.GetCount() }.context("Failed to get device count")?;
    info!("Active render endpoint count: {}", device_count);

    let own_pid = std::process::id();
    let mut sessions = Vec::new();

    for d in 0..device_count {
        // SAFETY: index d is within [0, device_count) as returned by GetCount.
        let device = match unsafe { devices.Item(d) } {
            Ok(dev) => dev,
            Err(e) => {
                warn!("Failed to get device {}: {}", d, e);
                continue;
            }
        };

        // SAFETY: Activate is a standard COM interface activation call.
        let session_manager: IAudioSessionManager2 =
            match unsafe { device.Activate(CLSCTX_ALL, None) } {
                Ok(sm) => sm,
                Err(e) => {
                    warn!("Failed to activate session manager for device {}: {}", d, e);
                    continue;
                }
            };

        // SAFETY: GetSessionEnumerator is a standard COM query.
        let session_enum: IAudioSessionEnumerator =
            match unsafe { session_manager.GetSessionEnumerator() } {
                Ok(se) => se,
                Err(e) => {
                    warn!("Failed to get session enumerator for device {}: {}", d, e);
                    continue;
                }
            };

        // SAFETY: GetCount is a simple getter with no invariants.
        let count = match unsafe { session_enum.GetCount() } {
            Ok(n) => n,
            Err(e) => {
                warn!("Failed to get session count for device {}: {}", d, e);
                continue;
            }
        };
        info!("Device {}: {} session(s)", d, count);

        for i in 0..count {
            // SAFETY: index i is within [0, count) as returned by GetCount.
            let control: IAudioSessionControl = match unsafe { session_enum.GetSession(i) } {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to get session {}/{}: {}", d, i, e);
                    continue;
                }
            };
            if let Some(session) = try_duck_session(control, own_pid, i) {
                sessions.push(session);
            }
        }
    }

    Ok(sessions)
}

/// Attempt to build a [`DuckedSession`] from a raw session control.
///
/// Returns `None` if the session should be skipped (own process, system
/// sounds, expired session, already silent, or missing COM interfaces).
/// PID=0 is NOT treated as expired: sandboxed processes (e.g. Chrome's audio
/// renderer) legitimately report PID=0 due to security restrictions.
#[cfg(windows)]
fn try_duck_session(
    control: IAudioSessionControl,
    own_pid: u32,
    idx: i32,
) -> Option<DuckedSession> {
    // Skip expired sessions by state, not PID.
    // SAFETY: GetState is a simple COM getter with no invariants.
    let state = match unsafe { control.GetState() } {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to get state for session {}: {}", idx, e);
            return None;
        }
    };
    if state == AudioSessionStateExpired {
        info!("Session {}: skipping expired session", idx);
        return None;
    }

    // SAFETY: cast is a standard COM QI call; safe as long as the interface is supported.
    let control2: IAudioSessionControl2 = match control.cast() {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "Session {} does not support IAudioSessionControl2: {}",
                idx, e
            );
            return None;
        }
    };

    // SAFETY: GetProcessId is a simple getter. Returns 0 for sandboxed processes.
    let pid = match unsafe { control2.GetProcessId() } {
        Ok(p) => p,
        Err(e) => {
            warn!("Failed to get PID for session {}: {}", idx, e);
            return None;
        }
    };

    // Skip our own process.
    if pid == own_pid {
        info!("Session {}: skipping own process (PID={})", idx, pid);
        return None;
    }

    // Skip Windows system sounds sessions.
    // IsSystemSoundsSession returns S_OK (0) for system sounds and S_FALSE (1)
    // for regular sessions. Both are non-error HRESULTs, so .is_ok() is wrong
    // here — it would match S_FALSE too, incorrectly skipping all sessions.
    // SAFETY: IsSystemSoundsSession is a simple COM query.
    if unsafe { control2.IsSystemSoundsSession() } == HRESULT(0) {
        info!("Session {}: skipping system sounds", idx);
        return None;
    }

    // SAFETY: cast is a standard COM QI call.
    let volume_control: ISimpleAudioVolume = match control2.cast() {
        Ok(v) => v,
        Err(e) => {
            warn!("Session {} does not support ISimpleAudioVolume: {}", idx, e);
            return None;
        }
    };

    // SAFETY: GetMasterVolume is a simple getter.
    let original_volume = match unsafe { volume_control.GetMasterVolume() } {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to get volume for session {}: {}", idx, e);
            return None;
        }
    };

    // Skip sessions already at zero to avoid a fade from 0 -> 0 -> 0.
    if original_volume == 0.0 {
        info!("Session {}: already silent, skipping", idx);
        return None;
    }

    info!(
        "Session {}: ducking (PID={}, original_volume={:.3}, state={:?})",
        idx, pid, original_volume, state
    );

    Some(DuckedSession {
        volume_control,
        original_volume,
    })
}
