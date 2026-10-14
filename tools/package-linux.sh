#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_HOME="${CARGO_HOME:-$PWD/.cargo-cache}"
cargo build --locked --offline --release -p stem-desktop -p stem-cli
package_dir="${1:-dist/stem-separator-linux-x86_64}"
if [[ -e "$package_dir" ]]; then
    echo "Package destination already exists: $package_dir" >&2
    exit 1
fi
mkdir -p "$package_dir/runtime" "$package_dir/models/htdemucs-6s" "$package_dir/licenses"
cp target/release/stem-desktop target/release/stem-cli "$package_dir/"
cp -L runtime/onnxruntime-linux-x64-1.22.0/lib/libonnxruntime*.so* "$package_dir/runtime/"
cp models/htdemucs-6s/{manifest.json,htdemucs_6s.onnx,LICENSE.txt} "$package_dir/models/htdemucs-6s/"
cp runtime/onnxruntime-linux-x64-1.22.0/{LICENSE,ThirdPartyNotices.txt} "$package_dir/licenses/"
cp README.md "$package_dir/README.md"
cp tools/launch-linux.sh "$package_dir/start.sh"
chmod +x "$package_dir/start.sh"
(
    cd "$package_dir"
    find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS
)
echo "Offline Linux app: $package_dir/start.sh"
