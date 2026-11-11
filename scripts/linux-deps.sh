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

apt_options=()
update_options=()
if [[ "${GITHUB_ACTIONS:-}" == true ]]; then
  # Avoid the hosted runner's Azure mirror, which can time out on each package.
  ubuntu_sources=/etc/apt/sources.list.d/ubuntu.sources
  if [[ -f "$ubuntu_sources" ]]; then
    "${privilege[@]}" sed -Ei \
      -e 's|mirror\+file:/etc/apt/apt-mirrors\.txt|https://archive.ubuntu.com/ubuntu/|g' \
      -e 's|https?://azure\.archive\.ubuntu\.com/ubuntu/?|https://archive.ubuntu.com/ubuntu/|g' \
      "$ubuntu_sources"
  fi
  apt_options=(-o Acquire::Retries=1 -o Acquire::http::Timeout=15 -o Acquire::https::Timeout=15)
  update_options=(--error-on=any)
fi

"${privilege[@]}" apt-get "${apt_options[@]}" update "${update_options[@]}"
"${privilege[@]}" apt-get "${apt_options[@]}" install -y \
  build-essential clang cmake pkg-config \
  libasound2-dev libfontconfig-dev libfreetype-dev libglib2.0-dev \
  libssl-dev libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev \
  libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libzstd-dev libvulkan-dev libvulkan1 libclang-dev \
  xdg-desktop-portal
