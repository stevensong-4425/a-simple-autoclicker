#!/usr/bin/env bash
set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
  build-essential \
  libadwaita-1-dev \
  libgtk-4-dev \
  libx11-dev \
  libxtst-dev \
  pkg-config

if ! command -v rustc >/dev/null || ! command -v cargo >/dev/null; then
  echo "Rust 1.88 or newer is also required. Install it from https://rustup.rs/"
  exit 1
fi

rust_version="$(rustc --version | awk '{print $2}')"
if [ "$(printf '%s\n' "1.88.0" "$rust_version" | sort -V | head -n 1)" != "1.88.0" ]; then
  echo "Rust $rust_version is too old. Install Rust 1.88 or newer from https://rustup.rs/"
  exit 1
fi

echo "Development dependencies installed (Rust $rust_version)."
