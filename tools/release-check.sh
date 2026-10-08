#!/usr/bin/env bash
# Checks that a release is consistent: the version in bhote, the herdr plugin and Omarchy manifests, the Arch PKGBUILD, the
# Homebrew formula and a CHANGELOG section all say the same. Usage: bash tools/release-check.sh <version>   (e.g. 0.5.9; CI runs it for every v* tag)
cd "$(dirname "$0")/.." || exit 1
want=${1#v}; [ -n "$want" ] || { echo "usage: tools/release-check.sh <version>" >&2; exit 2; }
v=$(sed -n 's/^VERSION=\${BHOTE_VERSION:-\(.*\)}$/\1/p' bhote)
p=$(sed -n 's/^pkgver=//p' packaging/arch/PKGBUILD)
f=$(sed -n 's/.*tag: "v\([^"]*\)".*/\1/p' packaging/homebrew/bhote.rb)
h=$(sed -n 's/^version = "\(.*\)"$/\1/p' herdr-plugin/herdr-plugin.toml)
o=$(sed -n 's/^ *"version": "\(.*\)",*$/\1/p' omarchy-plugin/manifest.json)
c=$(grep -c "^## \[$want\]" CHANGELOG.md)
echo "release $want · bhote $v · herdr plugin $h · Omarchy $o · PKGBUILD $p · formula $f · CHANGELOG section: $c"
[ "$v" = "$want" ] && [ "$h" = "$want" ] && [ "$o" = "$want" ] && [ "$p" = "$want" ] && [ "$f" = "$want" ] && [ "$c" = 1 ]
