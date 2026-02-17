# Download Whisper model files
# Usage: .\scripts\download-model.ps1 [model]
# Models: tiny, base, small, medium, large
# Default: base

param(
    [string]$Model = "base"
)

$ModelDir = "./assets/models"
$ModelFile = "ggml-$Model.bin"
$Url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$ModelFile"

# Create models directory if it doesn't exist
if (-not (Test-Path $ModelDir)) {
    New-Item -ItemType Directory -Path $ModelDir | Out-Null
    Write-Host "Created models directory"
}

# Check if model already exists
if (Test-Path "$ModelDir/$ModelFile") {
    Write-Host "Model already exists: $ModelDir/$ModelFile"
    $response = Read-Host "Do you want to re-download? (y/N)"
    if ($response -ne "y" -and $response -ne "Y") {
        Write-Host "Skipping download"
        exit 0
    }
}

Write-Host "Downloading Whisper $Model model from Hugging Face..."
Write-Host "URL: $Url"
Write-Host ""

try {
    # Download with progress
    $ProgressPreference = 'Continue'
    Invoke-WebRequest -Uri $Url -OutFile "$ModelDir/$ModelFile" -UseBasicParsing
    
    Write-Host ""
    Write-Host "Model downloaded successfully!" -ForegroundColor Green
    Write-Host "Location: $ModelDir/$ModelFile"
    
    # Show file size
    $fileSize = (Get-Item "$ModelDir/$ModelFile").Length / 1MB
    Write-Host "Size: $([math]::Round($fileSize, 2)) MB"
    
} catch {
    Write-Host ""
    Write-Host "Download failed: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "Available models:"
    Write-Host "  - tiny   (~75 MB)   - Fastest, lowest accuracy"
    Write-Host "  - base   (~140 MB)  - Recommended for CPU"
    Write-Host "  - small  (~460 MB)  - Better accuracy"
    Write-Host "  - medium (~1.5 GB)  - High accuracy"
    Write-Host "  - large  (~3 GB)    - Best accuracy"
    exit 1
}
