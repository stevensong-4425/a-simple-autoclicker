#!/usr/bin/env bash
set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
  build-essential \
  cargo \
  libadwaita-1-dev \
  libgtk-4-dev \
  libx11-dev \
  libxtst-dev \
  pkg-config \
  rustc

echo "Development dependencies installed."
