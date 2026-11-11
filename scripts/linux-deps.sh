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

packages=(
  build-essential clang cmake pkg-config
  libasound2-dev libfontconfig-dev libfreetype-dev libglib2.0-dev
  libssl-dev libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev
  libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
  libzstd-dev libvulkan-dev libvulkan1 libclang-dev
  xdg-desktop-portal
)

ubuntu_sources=/etc/apt/sources.list.d/ubuntu.sources
if [[ "${GITHUB_ACTIONS:-}" != true || ! -f "$ubuntu_sources" ]]; then
  "${privilege[@]}" apt-get update
  "${privilege[@]}" apt-get install -y "${packages[@]}"
  exit
fi

# Keep update and install on the same fresh lists and the runner's Ubuntu
# signing keyring, without reusing a machine-wide package cache.
. /etc/os-release
apt_dir="$(mktemp -d /tmp/force-apt.XXXXXX)"
trap '"${privilege[@]}" rm -rf -- "$apt_dir"' EXIT
chmod 0755 "$apt_dir"
mkdir "$apt_dir/lists" "$apt_dir/sources.list.d"
apt_options=(
  -o "Dir::Etc::sourcelist=$apt_dir/ubuntu.sources"
  -o "Dir::Etc::sourceparts=$apt_dir/sources.list.d"
  -o "Dir::State::lists=$apt_dir/lists"
  -o Dir::Cache::pkgcache= -o Dir::Cache::srcpkgcache=
  -o Acquire::Retries=1 -o Acquire::http::Timeout=15 -o Acquire::https::Timeout=15
)

for archive in https://archive.ubuntu.com/ubuntu \
  https://mirrors.edge.kernel.org/ubuntu \
  https://mirror.math.princeton.edu/pub/ubuntu; do
  if ! "${privilege[@]}" curl --fail --silent --show-error --location --head \
    --proto '=https' --proto-redir '=https' --connect-timeout 4 --max-time 8 \
    --output /dev/null "$archive/dists/$VERSION_CODENAME-security/InRelease"; then
    continue
  fi

  printf 'Checking Ubuntu archive %s\n' "$archive"
  sed -E \
    -e "s|mirror\+file:/etc/apt/apt-mirrors\.txt|$archive/|g" \
    -e "s#https?://(azure\.archive|archive|security)\.ubuntu\.com/ubuntu/?#$archive/#g" \
    "$ubuntu_sources" > "$apt_dir/ubuntu.sources"
  if ! "${privilege[@]}" apt-get "${apt_options[@]}" update --error-on=any; then
    continue
  fi
  if "${privilege[@]}" apt-get "${apt_options[@]}" install --simulate "${packages[@]}" > "$apt_dir/plan" 2>&1; then
    "${privilege[@]}" apt-get "${apt_options[@]}" install -y "${packages[@]}"
    exit
  fi
  tail -n 30 "$apt_dir/plan" >&2
done

echo 'No Ubuntu archive mirror provided a complete set of build dependencies.' >&2
exit 1
