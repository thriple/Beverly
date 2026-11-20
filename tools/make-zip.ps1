# Make Beverly-windows.zip on your own PC - the same zip the GitHub build publishes.
# Needs Rust (https://rustup.rs) and the Visual Studio C++ build tools that rustup offers to install.
# Run from the repository folder:  powershell -ExecutionPolicy Bypass -File tools\make-zip.ps1
$ErrorActionPreference = "Stop"
Push-Location editor
cargo build --release
Pop-Location
Remove-Item -Recurse -Force dist -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force dist/Beverly | Out-Null
Copy-Item editor/target/release/beverly.exe dist/Beverly/Beverly.exe
Copy-Item editor/target/release/beverly-player.exe dist/Beverly/beverly-player.exe
Copy-Item editor/README-windows.txt dist/Beverly/README.txt
Compress-Archive -Force -Path dist/Beverly -DestinationPath Beverly-windows.zip
Write-Host "Made Beverly-windows.zip"
# the benchmark, its own small download
New-Item -ItemType Directory -Force bench/Beverly-bench | Out-Null
Copy-Item editor/target/release/beverly-bench.exe bench/Beverly-bench/beverly-bench.exe
Copy-Item editor/README-bench.txt bench/Beverly-bench/README.txt
Compress-Archive -Force -Path bench/Beverly-bench -DestinationPath Beverly-bench-windows.zip
Write-Host "Made Beverly-bench-windows.zip"
