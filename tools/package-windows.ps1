param([string]$Destination = "dist/stem-separator-windows-x86_64")
$ErrorActionPreference = "Stop"
if ($env:OS -ne "Windows_NT") { throw "Run this packager on Windows with the Rust MSVC toolchain." }
Set-Location (Join-Path $PSScriptRoot "..")
if (-not $env:CARGO_HOME) { $env:CARGO_HOME = Join-Path (Get-Location) ".cargo-cache" }
if (Test-Path $Destination) { throw "Package destination already exists: $Destination" }
$Runtime = "runtime/onnxruntime-win-x64-1.22.0"
if (-not (Test-Path "$Runtime/lib/onnxruntime.dll")) { throw "Provision ONNX Runtime 1.22.0 Windows x64 under $Runtime first." }
cargo build --locked --offline --release -p stem-desktop -p stem-cli
if ($LASTEXITCODE -ne 0) { throw "Release build failed" }
New-Item -ItemType Directory -Path "$Destination/runtime", "$Destination/models/htdemucs-6s", "$Destination/licenses" | Out-Null
Copy-Item target/release/stem-desktop.exe, target/release/stem-cli.exe $Destination
Copy-Item "$Runtime/lib/*.dll" "$Destination/runtime"
Copy-Item models/htdemucs-6s/manifest.json, models/htdemucs-6s/htdemucs_6s.onnx, models/htdemucs-6s/LICENSE.txt "$Destination/models/htdemucs-6s"
Copy-Item "$Runtime/LICENSE", "$Runtime/ThirdPartyNotices.txt" "$Destination/licenses"
Copy-Item README.md "$Destination/README.md"
$Root = (Resolve-Path $Destination).Path
Get-ChildItem $Root -File -Recurse | Sort-Object FullName | ForEach-Object {
    $Relative = $_.FullName.Substring($Root.Length + 1).Replace('\', '/')
    "$( (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() )  $Relative"
} | Set-Content "$Destination/SHA256SUMS" -Encoding ASCII
Write-Output "Windows app: $Destination/stem-desktop.exe (platform verification required)"
