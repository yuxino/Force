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
  # Probe once so a runner DNS failure does not stall every index and package.
  # Use Ubuntu archive mirrors; keep the image's suites and signing keyring.
  ubuntu_sources=/etc/apt/sources.list.d/ubuntu.sources
  if [[ -f "$ubuntu_sources" ]]; then
    . /etc/os-release
    archive=
    for candidate in https://archive.ubuntu.com/ubuntu \
      https://mirrors.edge.kernel.org/ubuntu \
      https://mirror.math.princeton.edu/pub/ubuntu; do
      if "${privilege[@]}" curl --fail --silent --show-error --location --head \
        --proto '=https' --proto-redir '=https' --connect-timeout 4 --max-time 8 \
        --output /dev/null "$candidate/dists/$VERSION_CODENAME-security/InRelease"; then
        archive="$candidate"
        break
      fi
    done
    if [[ -z "$archive" ]]; then
      echo 'No Ubuntu archive mirror is reachable from this runner.' >&2
      exit 1
    fi
    printf 'Using Ubuntu archive %s\n' "$archive"
    "${privilege[@]}" sed -Ei \
      -e "s|mirror\+file:/etc/apt/apt-mirrors\.txt|$archive/|g" \
      -e "s#https?://(azure\.archive|archive|security)\.ubuntu\.com/ubuntu/?#$archive/#g" \
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
