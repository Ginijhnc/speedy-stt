# Script to copy notification.mp3 and create a lower-pitched version
# Requires FFmpeg to be installed and available in PATH

$sourceFile = "assets/sounds/notification.mp3"
$outputFile = "assets/sounds/notification-low.mp3"

# Check if FFmpeg is available
try {
    $null = Get-Command ffmpeg -ErrorAction Stop
} catch {
    Write-Host "Error: FFmpeg is not installed or not in PATH" -ForegroundColor Red
    Write-Host "Please install FFmpeg from https://ffmpeg.org/download.html" -ForegroundColor Yellow
    exit 1
}

# Check if source file exists
if (-not (Test-Path $sourceFile)) {
    Write-Host "Error: Source file '$sourceFile' not found" -ForegroundColor Red
    exit 1
}

Write-Host "Creating lower-pitched version of notification sound..." -ForegroundColor Cyan

# Lower pitch by 3 semitones (adjust the value to taste)
# asetrate increases sample rate (makes it faster/higher), then atempo slows it back down
# This preserves duration while lowering pitch
ffmpeg -i $sourceFile -af "asetrate=44100*0.8909,atempo=1.1225" -y $outputFile

if ($LASTEXITCODE -eq 0) {
    Write-Host "Success! Created: $outputFile" -ForegroundColor Green
    Write-Host "Original: $sourceFile" -ForegroundColor Gray
    Write-Host "Lower pitch: $outputFile" -ForegroundColor Gray
} else {
    Write-Host "Error: FFmpeg failed to process the file" -ForegroundColor Red
    exit 1
}
