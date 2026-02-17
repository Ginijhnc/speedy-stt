# Check for LLVM installation by trying to find clang
$llvmPath = $null

# Try to find clang in PATH
try {
    $clangExe = Get-Command clang -ErrorAction Stop
    $llvmPath = Split-Path $clangExe.Source -Parent
    Write-Host "Found LLVM at: $llvmPath" -ForegroundColor Green
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
    exit 1
}

$env:LIBCLANG_PATH = $llvmPath
Write-Host "Set LIBCLANG_PATH=$env:LIBCLANG_PATH" -ForegroundColor Cyan

# Load VS 2022 environment
& "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\Launch-VsDevShell.ps1" -Arch amd64 -HostArch amd64

$env:CMAKE_GENERATOR = "NMake Makefiles"

cargo clippy --all-targets -- -D warnings
