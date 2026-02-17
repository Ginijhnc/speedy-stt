//! System tray icon management

use anyhow::{Context, Result};
use tracing::info;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

/// System tray icon states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    /// Idle state
    Idle,
    /// Recording state
    Recording,
}

/// System tray manager
pub struct TrayManager {
    /// Tray icon
    tray: TrayIcon,
    /// Quit menu item
    quit_item: MenuItem,
    /// Idle icon
    idle_icon: Option<Icon>,
    /// Recording icon
    recording_icon: Option<Icon>,
}

impl TrayManager {
    /// Create new tray manager
    pub fn new() -> Result<Self> {
        let quit_item = MenuItem::new("Quit", true, None);
        let menu = Menu::new();
        menu.append(&quit_item).context("Failed to add quit item")?;

        // Try to load icons (optional - will use default if not found)
        let idle_icon = Self::load_icon("./assets/icons/microphone.ico");
        let recording_icon = Self::load_icon("./assets/icons/microphone.ico");

        let mut builder = TrayIconBuilder::new()
            .with_tooltip("Speedy STT")
            .with_menu(Box::new(menu));

        // Set initial icon if available
        if let Some(ref icon) = idle_icon {
            builder = builder.with_icon(icon.clone());
        }

        let tray = builder.build().context("Failed to create tray icon")?;

        info!("System tray icon created");

        Ok(Self {
            tray,
            quit_item,
            idle_icon,
            recording_icon,
        })
    }

    /// Load icon from file
    fn load_icon(path: &str) -> Option<Icon> {
        match Icon::from_path(path, None) {
            Ok(icon) => {
                info!("Loaded icon: {}", path);
                Some(icon)
            }
            Err(e) => {
                info!("Could not load icon {}: {} (using default)", path, e);
                None
            }
        }
    }

    /// Update tray icon state
    pub fn set_state(&mut self, state: TrayState) -> Result<()> {
        let tooltip = match state {
            TrayState::Idle => "Speedy STT - Idle",
            TrayState::Recording => "Speedy STT - Recording",
        };

        self.tray
            .set_tooltip(Some(tooltip))
            .context("Failed to set tooltip")?;

        // Update icon if available
        let icon = match state {
            TrayState::Idle => &self.idle_icon,
            TrayState::Recording => &self.recording_icon,
        };

        if let Some(icon) = icon {
            self.tray.set_icon(Some(icon.clone()))?;
        }

        info!("Tray state updated: {:?}", state);

        Ok(())
    }

    /// Check if quit was clicked
    pub fn should_quit(&self) -> bool {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            return event.id == self.quit_item.id();
        }
        false
    }
}
