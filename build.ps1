# Check for LLVM installation by trying to find clang
$llvmPath = $null

# Try to find clang in PATH
try {
    $clangExe = Get-Command clang -ErrorAction Stop
    $llvmPath = Split-Path $clangExe.Source -Parent
    Write-Host "Found LLVM at: $llvmPath" -ForegroundColor Green
    $clangVersion = & clang --version 2>&1
    Write-Host "$($clangVersion[0])" -ForegroundColor Green
} catch {
    # Try common installation paths
    $commonPaths = @(
        "C:\Program Files\LLVM\bin",
        "C:\Program Files (x86)\LLVM\bin",
        "D:\Programs\LLVM\bin",
        "$env:ProgramFiles\LLVM\bin",
        "${env:ProgramFiles(x86)}\LLVM\bin"
    )
    
    foreach ($path in $commonPaths) {
        if (Test-Path "$path\clang.exe") {
            $llvmPath = $path
            Write-Host "Found LLVM at: $llvmPath (not in PATH)" -ForegroundColor Yellow
            $env:PATH = "$llvmPath;$env:PATH"
            break
        }
    }
}

if (-not $llvmPath) {
    Write-Host "ERROR: LLVM not found" -ForegroundColor Red
    Write-Host ""
    Write-Host "LLVM is required to build this project. Please install it:" -ForegroundColor Yellow
    Write-Host "1. Download from: https://github.com/llvm/llvm-project/releases/latest" -ForegroundColor Yellow
    Write-Host "2. Get: LLVM-<version>-win64.exe" -ForegroundColor Yellow
    Write-Host "3. During installation, check 'Add LLVM to the system PATH'" -ForegroundColor Yellow
    Write-Host "4. Restart your terminal after installation" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Or if already installed, add it to PATH manually." -ForegroundColor Yellow
    Write-Host "See SETUP.md for detailed instructions." -ForegroundColor Yellow
    exit 1
}

# Set LIBCLANG_PATH to help bindgen find libclang.dll
$env:LIBCLANG_PATH = $llvmPath
Write-Host "Set LIBCLANG_PATH=$env:LIBCLANG_PATH" -ForegroundColor Cyan

# Load VS 2022 environment
& "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\Launch-VsDevShell.ps1" -Arch amd64 -HostArch amd64

# Use NMake Makefiles generator which works better with incomplete VS installations
$env:CMAKE_GENERATOR = "NMake Makefiles"

# Build with verbose output
cargo build --release -vv

# Workaround: Copy whisper.lib to expected location if build succeeded
$whisperLibPath = "target\release\build\whisper-rs-sys-*\out\lib\static\whisper.lib"
$targetPath = "target\release\build\whisper-rs-sys-*\out\whisper.lib"

if (Test-Path $whisperLibPath) {
    $sourceLib = Get-Item $whisperLibPath | Select-Object -First 1
    $targetDir = Split-Path (Get-Item $targetPath -ErrorAction SilentlyContinue | Select-Object -First 1).FullName -Parent
    
    if ($targetDir -and $sourceLib) {
        Copy-Item $sourceLib.FullName "$targetDir\whisper.lib" -Force
        Write-Host "Copied whisper.lib to linker search path"
        
        # Retry build if it failed
        cargo build --release
    }
}
