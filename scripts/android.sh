#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
# 复用已安装的工具链；安装 SDK 和接受许可由开发者自己完成。
export ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
export NDK_HOME="${NDK_HOME:-${ANDROID_NDK_HOME:-${ANDROID_HOME:+$ANDROID_HOME/ndk/28.2.13676358}}}"
if [[ -z "$ANDROID_HOME" || ! -d "$NDK_HOME" ]]; then
  printf '%s\n' '请设置 ANDROID_HOME，并安装 NDK 28.2.13676358；自定义 NDK 路径可设置 NDK_HOME。' >&2
  exit 1
fi
export ANDROID_NDK_HOME="$NDK_HOME"
# Tauri 设置各 ABI 的编译器，OpenSSL 的构建子进程还需要从 PATH 找到 LLVM 工具。
ndk_bins=("$NDK_HOME"/toolchains/llvm/prebuilt/*/bin)
if [[ ! -x "${ndk_bins[0]}/clang" ]]; then
  printf '%s\n' '当前 NDK 缺少可执行的 LLVM 工具，请检查 NDK_HOME。' >&2
  exit 1
fi
export PATH="${ndk_bins[0]}:$PATH"
exec npm run tauri -- android "$@"
