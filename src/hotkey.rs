//! Global hotkey handling

use anyhow::{Context, Result};
use global_hotkey::{
    GlobalHotKeyManager,
    hotkey::{Code, HotKey, Modifiers},
};
use tracing::info;

/// Hotkey listener
pub struct HotkeyListener {
    /// Hotkey manager
    _manager: GlobalHotKeyManager,
    /// Hotkey ID
    pub hotkey: HotKey,
}

impl HotkeyListener {
    /// Create new hotkey listener
    pub fn new(modifier: &str, key: &str) -> Result<Self> {
        let manager = GlobalHotKeyManager::new().context("Failed to create hotkey manager")?;

        let modifiers = if modifier.is_empty() || modifier.to_uppercase() == "NONE" {
            None
        } else {
            Some(Self::parse_modifier(modifier)?)
        };

        let code = Self::parse_key(key)?;

        let hotkey = HotKey::new(modifiers, code);

        // Try to register the hotkey
        match manager.register(hotkey) {
            Ok(_) => {
                let hotkey_desc = if modifier.is_empty() || modifier.to_uppercase() == "NONE" {
                    key.to_string()
                } else {
                    format!("{} + {}", modifier, key)
                };
                info!("Registered hotkey: {}", hotkey_desc);
                Ok(Self {
                    _manager: manager,
                    hotkey,
                })
            }
            Err(_e) => {
                anyhow::bail!(
                    "Failed to register hotkey {} + {}. This combination may be reserved by Windows. \
                    Try a different combination like CTRL+SPACE or ALT+SPACE in your .env file.",
                    modifier,
                    key
                )
            }
        }
    }

    /// Parse modifier string to Modifiers
    fn parse_modifier(modifier: &str) -> Result<Modifiers> {
        match modifier.to_uppercase().as_str() {
            "CTRL" => Ok(Modifiers::CONTROL),
            "ALT" => Ok(Modifiers::ALT),
            "SHIFT" => Ok(Modifiers::SHIFT),
            "WIN" | "SUPER" => Ok(Modifiers::SUPER),
            _ => anyhow::bail!("Invalid modifier: {}", modifier),
        }
    }

    /// Parse key string to Code
    fn parse_key(key: &str) -> Result<Code> {
        match key.to_uppercase().as_str() {
            "WIN" | "SUPER" => Ok(Code::MetaLeft),
            "ALT" => Ok(Code::AltLeft),
            "ALTRIGHT" => Ok(Code::AltRight),
            "SPACE" => Ok(Code::Space),
            "ENTER" | "RETURN" => Ok(Code::Enter),
            "TAB" => Ok(Code::Tab),
            "BACKSPACE" => Ok(Code::Backspace),
            "ESC" | "ESCAPE" => Ok(Code::Escape),
            "F1" => Ok(Code::F1),
            "F2" => Ok(Code::F2),
            "F3" => Ok(Code::F3),
            "F4" => Ok(Code::F4),
            "F5" => Ok(Code::F5),
            "F6" => Ok(Code::F6),
            "F7" => Ok(Code::F7),
            "F8" => Ok(Code::F8),
            "F9" => Ok(Code::F9),
            "F10" => Ok(Code::F10),
            "F11" => Ok(Code::F11),
            "F12" => Ok(Code::F12),
            "A" => Ok(Code::KeyA),
            "B" => Ok(Code::KeyB),
            "C" => Ok(Code::KeyC),
            "D" => Ok(Code::KeyD),
            "E" => Ok(Code::KeyE),
            "F" => Ok(Code::KeyF),
            "G" => Ok(Code::KeyG),
            "H" => Ok(Code::KeyH),
            "I" => Ok(Code::KeyI),
            "J" => Ok(Code::KeyJ),
            "K" => Ok(Code::KeyK),
            "L" => Ok(Code::KeyL),
            "M" => Ok(Code::KeyM),
            "N" => Ok(Code::KeyN),
            "O" => Ok(Code::KeyO),
            "P" => Ok(Code::KeyP),
            "Q" => Ok(Code::KeyQ),
            "R" => Ok(Code::KeyR),
            "S" => Ok(Code::KeyS),
            "T" => Ok(Code::KeyT),
            "U" => Ok(Code::KeyU),
            "V" => Ok(Code::KeyV),
            "W" => Ok(Code::KeyW),
            "X" => Ok(Code::KeyX),
            "Y" => Ok(Code::KeyY),
            "Z" => Ok(Code::KeyZ),
            _ => anyhow::bail!("Invalid key: {}", key),
        }
    }
}
