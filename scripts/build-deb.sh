#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
architecture="$(dpkg --print-architecture)"
package_root="$(mktemp -d)"
trap 'rm -rf "$package_root"' EXIT
chmod 0755 "$package_root"

cargo build --release

install -Dm755 \
  target/release/a-simple-autoclicker \
  "$package_root/usr/bin/a-simple-autoclicker"
install -Dm644 \
  data/com.asimpleautoclicker.App.desktop \
  "$package_root/usr/share/applications/com.asimpleautoclicker.App.desktop"
install -Dm644 README.md \
  "$package_root/usr/share/doc/a-simple-autoclicker/README.md"
install -Dm644 LICENSE \
  "$package_root/usr/share/doc/a-simple-autoclicker/copyright"
install -d "$package_root/DEBIAN" dist
sed \
  -e "s/@VERSION@/$version/g" \
  -e "s/@ARCH@/$architecture/g" \
  packaging/debian/control.in > "$package_root/DEBIAN/control"

output="dist/a-simple-autoclicker_${version}_${architecture}.deb"
dpkg-deb --root-owner-group --build "$package_root" "$output"
echo "Built $output"
