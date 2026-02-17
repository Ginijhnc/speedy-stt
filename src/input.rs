//! Text injection via keyboard simulation

use anyhow::Result;
use enigo::{Enigo, Keyboard, Settings};
use std::thread;
use std::time::Duration;

/// Text injector
pub struct TextInjector {
    /// Enigo instance
    enigo: Enigo,
}

impl TextInjector {
    /// Create new text injector
    pub fn new() -> Self {
        Self {
            enigo: Enigo::new(&Settings::default()).expect("Failed to create Enigo instance"),
        }
    }

    /// Type text into active window
    pub fn inject(&mut self, text: &str) -> Result<()> {
        thread::sleep(Duration::from_millis(100));

        // Use text() method which is more reliable for Unicode on Windows
        self.enigo.text(text)?;

        Ok(())
    }
}

impl Default for TextInjector {
    fn default() -> Self {
        Self::new()
    }
}
