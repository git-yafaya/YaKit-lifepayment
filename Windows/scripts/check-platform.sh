#!/usr/bin/env bash
# 单独检查 Windows API 类型，避免 Linux 缺少 SQLCipher 的 MSVC 原生工具链。
set -euo pipefail
project_root="$(cd "$(dirname "$0")/../.." && pwd)"
check_dir="$(mktemp -d)"
trap 'rm -rf "$check_dir"' EXIT
mkdir -p "$check_dir/src"
cat > "$check_dir/Cargo.toml" <<'MANIFEST'
[package]
name="lightledger-platform-check"
version="0.1.0"
edition="2024"
[dependencies]
serde_json="=1.0.149"
windows-future="=0.3.2"
windows={version="=0.62.2",features=["ApplicationModel","Foundation","Foundation_Collections","Globalization","Graphics_Imaging","Media_Ocr","Storage","Storage_Streams","Security_Credentials_UI","UI_Notifications","UI_Notifications_Management","Win32_Foundation","Win32_System_WinRT"]}
MANIFEST
printf '#[path="%s/Windows/src-tauri/src/platform/windows.rs"]\npub mod platform;\n' "$project_root" > "$check_dir/src/lib.rs"
cargo check --manifest-path "$check_dir/Cargo.toml" --target x86_64-pc-windows-msvc
