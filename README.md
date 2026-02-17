# Speedy-STT

Minimal speech-to-text dictation app using [whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp) as the underlying model. It runs in the systray with no UI and is activated via a hotkey. As of right now, Windows 10+ is the only supported OS.

This project was made using AI, specifically Opus 4.6 (claude-opus-4-6) for implementation planning and Sonnet 4.5 (claude-sonnet-4-5-20250929) for execution using the Claude Code VSCode extension.

## Why This Exists

a) Model benchmarking purposes.

b) Saves me 12 USD per month by avoiding a monthly subscription to proprietary alternatives.

c) Wanted to quickly build my own STT app, with no UI/features I don't use. Lowest resource consumption possible.

## Features

- Customizable global hotkey activation. Hold to record, release to transcribe
- System tray icon
- Audio feedback (beep sounds for start & finish)
- Volume boost for distant microphones
- Whisper-based transcription (CPU-optimized)
- Auto-paste transcribed text into active window
- Lazy model loading: model is loaded on demand and freed after a configurable idle cooldown

## Quick Start

```powershell
# 1. Install LLVM (REQUIRED)
# Download from: https://github.com/llvm/llvm-project/releases/latest
# Get: LLVM-<version>-win64.exe
# IMPORTANT: Check "Add LLVM to the system PATH" during installation

# 2. Verify LLVM is installed
clang --version

# 3. Download model
.\assets\scripts\download-model.ps1 base

# 4. Build and run
.\build.ps1
.\target\release\speedy-stt.exe
```

See [SETUP.md](SETUP.md) for detailed instructions.

## Requirements

- Windows 10+
- Rust 1.85+
- LLVM/Clang
- Visual Studio 2022 (Community edition or Build Tools with C++ workload)

## Configuration

Edit `.env` to customize hotkeys, volume boost, models, etc. See `.env.example` for options.

## Development

```powershell
.\build.ps1                                            # Build release binary
.\clippy.ps1                                           # Linting with Clippy
cargo fmt --all                                        # Format
cargo fmt --all --check                                # Check formatting
cargo check                                            # Type check
cargo audit                                            # Security audit
cargo deny check advisories licenses bans sources      # Dependency policy
```
