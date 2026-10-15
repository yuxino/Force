#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != Linux ]] || ! command -v apt-get >/dev/null 2>&1; then
  echo 'This script supports Debian and Ubuntu. See README.md for other systems.' >&2
  exit 1
fi

privilege=()
if [[ "$(id -u)" -ne 0 ]]; then
  privilege=(sudo)
fi

"${privilege[@]}" apt-get update
"${privilege[@]}" apt-get install -y \
  build-essential clang cmake pkg-config \
  libasound2-dev libfontconfig-dev libfreetype-dev libglib2.0-dev \
  libssl-dev libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev \
  libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libzstd-dev libvulkan-dev libvulkan1 libclang-dev \
  xdg-desktop-portal
