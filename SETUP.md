# Setup Guide

## Prerequisites

### Visual Studio 2022

Visual Studio is required to build the Whisper C++ library.

#### Option 1: Visual Studio 2022 Community (Recommended)

1. Download [Visual Studio 2022 Community](https://visualstudio.microsoft.com/downloads/)
2. Run the installer
3. Select "Desktop development with C++" workload
4. Click Install (requires ~7GB disk space)

#### Option 2: Build Tools Only (Smaller)

1. Download [Build Tools for Visual Studio 2022](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)
2. Run the installer
3. Select "C++ build tools" workload
4. Click Install (requires ~4GB disk space)

## Installing LLVM

LLVM is required to build the Whisper bindings.

### Download and Install

1. Go to [LLVM releases](https://github.com/llvm/llvm-project/releases/latest)
2. Download `LLVM-<version>-win64.exe` (~500MB)
   - Example: `LLVM-17.0.6-win64.exe`
3. Run the installer
4. **Important**: During installation, check "Add LLVM to the system PATH for all users"
5. Complete the installation
6. Restart your terminal (or reboot if needed)

### Verify Installation

Open a new PowerShell window and run:

```powershell
clang --version
```

You should see output like: `clang version 17.0.0` or similar.

### If PATH wasn't added automatically

If `clang --version` doesn't work, manually add to PATH:

1. Open System Properties → Environment Variables
2. Under System Variables, find `Path`
3. Click Edit → New
4. Add: `C:\Program Files\LLVM\bin`
5. Click OK and restart your terminal

Or add temporarily to current session:

```powershell
$env:PATH += ";C:\Program Files\LLVM\bin"
clang --version
```

## Disk Space Requirements

- Visual Studio 2022: ~4-7GB (depending on edition)
- LLVM: ~2GB installed
- Rust toolchain: ~1-2GB
- Project build artifacts: ~500MB-1GB
- Whisper models:
  - tiny: ~75MB
  - base: ~140MB
  - small: ~460MB
  - medium: ~1.5GB
  - large: ~3GB

## Building the Project

### First Time Setup

1. Install Visual Studio 2022 (see above)
2. Install LLVM (see above)
3. Copy `.env.example` to `.env`:
   ```powershell
   Copy-Item .env.example .env
   ```
4. Download Whisper model:
   ```powershell
   .\assets\scripts\download-model.ps1 base
   ```
5. Build using the provided script:
   ```powershell
   .\build.ps1
   ```
   First build takes 5-10 minutes.

### Running

```powershell
.\target\release\speedy-stt.exe
```

### Why use build.ps1?

The `build.ps1` script sets up the correct Visual Studio environment and CMake generator needed to compile the Whisper C++ library. It's more reliable than running `cargo build` directly.

## Troubleshooting

### "Visual Studio 17 2022 could not find any instance"

Your Visual Studio installation may be incomplete. Solutions:

1. **Complete the installation** (Recommended):
   - Open Visual Studio Installer
   - Click "Modify" on Visual Studio 2022
   - Ensure "Desktop development with C++" workload is checked
   - Click "Modify" to complete installation

2. **Use the build script**:
   - The `build.ps1` script works around this issue
   - Always use `.\build.ps1` instead of `cargo build`

### "Unable to find libclang" or "couldn't find any valid shared libraries matching: ['clang.dll', 'libclang.dll']"

This error means LLVM is not installed or not in your PATH. This is the most common build error.

**Solution:**

1. Download LLVM from [GitHub releases](https://github.com/llvm/llvm-project/releases/latest)
2. Get `LLVM-<version>-win64.exe` (e.g., LLVM-17.0.6-win64.exe) and run installer
3. **IMPORTANT**: During installation, check "Add LLVM to system PATH for all users"
4. Restart your terminal (or reboot if needed)
5. Verify installation: `clang --version`
6. If still failing, manually add to PATH: `C:\Program Files\LLVM\bin`

**Quick fix if LLVM is installed but not in PATH:**

```powershell
$env:PATH += ";C:\Program Files\LLVM\bin"
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
.\build.ps1
```

The updated `build.ps1` script now checks for LLVM and sets `LIBCLANG_PATH` automatically.

### CMake or build errors

- Ensure Visual Studio 2022 is fully installed with C++ workload
- Always use `.\build.ps1` instead of `cargo build` directly
- Try cleaning and rebuilding:
  ```powershell
  cargo clean
  .\build.ps1
  ```

### Model download fails

- Use assets\scripts\download-model.ps1 or download manually from [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp/tree/main)
- Place in `assets/models/` folder as `ggml-base.bin`

### No audio captured

- Check microphone permissions in Windows Settings
- Verify default input device is correct
- Increase `VOLUME_BOOST` in `.env` if mic is quiet

### Hotkey doesn't work

- Check for conflicts with other apps
- Try different key combination in `.env`
