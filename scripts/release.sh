#!/usr/bin/env bash
# Local CI and release; replaces the GitHub Actions workflows.
#
#   scripts/release.sh             checks, browser tests, builds, smoke tests, packages
#   scripts/release.sh --publish   ...then tags v<version>, pushes and creates the GitHub Release
#
# Environment:
#   ZIG_HOME    directory with the zig 0.14.1 binary (default ~/.local/zig-0.14.1)
#   SMOKE_HOST  user@host of an x86_64 Linux machine for the Linux smoke test (optional)
#
# Requires an Apple Silicon Mac with Rust (rust-toolchain.toml), Node, Python 3,
# cargo-zigbuild, Zig 0.14.1, gh and Playwright Chromium. macOS arm64 is built
# natively, macOS x86_64 as a cross build, Linux x86_64/aarch64 and Windows x86_64
# with cargo-zigbuild (Linux against glibc 2.28, Windows with the GNU toolchain).
# Each asset gets a .manifest.json; builds that cannot run here say so in it.
set -euo pipefail
cd "$(dirname "$0")/.."

publish=0
for arg in "$@"; do
  case "$arg" in
    --publish) publish=1 ;;
    *) echo "usage: scripts/release.sh [--publish]" >&2; exit 2 ;;
  esac
done

step() { printf '\n==> %s\n' "$*"; }
die() { printf 'release: %s\n' "$*" >&2; exit 1; }

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
tag="v$version"
out="target/release-$version"
repo=zhangchaosd/bookmarkd
# asset:cargo target (as passed to cargo / cargo zigbuild)
platforms=(
  "bookmarkd-macos-arm64:aarch64-apple-darwin"
  "bookmarkd-macos-x86_64:x86_64-apple-darwin"
  "bookmarkd-linux-x86_64:x86_64-unknown-linux-gnu.2.28"
  "bookmarkd-linux-aarch64:aarch64-unknown-linux-gnu.2.28"
  "bookmarkd-windows-x86_64.exe:x86_64-pc-windows-gnu"
)

step "Preconditions for $tag"
[ -z "$(git status --porcelain)" ] || die "working tree is not clean"
grep -q "\"version\": \"$version\"" web/package.json || die "web/package.json is not $version"
if [ "$publish" = 1 ]; then
  [ "$(git branch --show-current)" = main ] || die "publish from main"
  git fetch -q origin
  git merge-base --is-ancestor origin/main HEAD || die "main is behind origin/main; pull first"
  ! git rev-parse -q --verify "refs/tags/$tag" >/dev/null || die "tag $tag already exists"
  ! git ls-remote --exit-code --tags origin "$tag" >/dev/null 2>&1 || die "tag $tag already exists on origin"
fi
export PATH="${ZIG_HOME:-$HOME/.local/zig-0.14.1}:$PATH"
[ "$(zig version 2>/dev/null)" = 0.14.1 ] || die "zig 0.14.1 not found (set ZIG_HOME)"
command -v cargo-zigbuild >/dev/null || die "cargo-zigbuild not installed"
rustup target add aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-gnu \
  aarch64-unknown-linux-gnu x86_64-pc-windows-gnu >/dev/null

step "Checks (what CI ran)"
npm ci --prefix web
npm run check --prefix web
npm run build --prefix web
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace

step "Build"
for entry in "${platforms[@]}"; do
  target=${entry#*:}
  case "$target" in
    *apple-darwin) cargo build --locked --release --target "$target" ;;
    *) cargo zigbuild --locked --release --target "$target" ;;
  esac
done

step "Browser tests against the release binary"
(cd web && BOOKMARKD_BIN="$PWD/../target/aarch64-apple-darwin/release/bookmarkd" npx playwright test)

step "Package and smoke test $out"
rm -rf "$out"
mkdir -p "$out"
assets=()
for entry in "${platforms[@]}"; do
  asset=${entry%%:*}
  target=${entry#*:}
  target=${target%.2.28}
  binary=bookmarkd
  [[ "$asset" == *.exe ]] && binary=bookmarkd.exe
  cp "target/$target/release/$binary" "$out/$asset"
  assets+=("$asset" "$asset.manifest.json")
done
# Native build: the full smoke test writes its own manifest.
python3 scripts/release_check.py "$out/bookmarkd-macos-arm64" >/dev/null
echo "bookmarkd-macos-arm64: smoke ok"
# Linux x86_64 on a real machine when one is available.
if [ -n "${SMOKE_HOST:-}" ]; then
  remote=$(ssh "$SMOKE_HOST" 'mktemp -d')
  scp -q "$out/bookmarkd-linux-x86_64" scripts/release_check.py "$SMOKE_HOST:$remote/"
  ssh "$SMOKE_HOST" "chmod +x $remote/bookmarkd-linux-x86_64 && python3 $remote/release_check.py $remote/bookmarkd-linux-x86_64 >/dev/null"
  scp -q "$SMOKE_HOST:$remote/bookmarkd-linux-x86_64.manifest.json" "$out/"
  ssh "$SMOKE_HOST" "rm -rf $remote"
  echo "bookmarkd-linux-x86_64: smoke ok on $SMOKE_HOST"
fi
# Everything else cannot run here: record what was checked instead of a smoke result.
for asset in bookmarkd-macos-x86_64 bookmarkd-linux-x86_64 bookmarkd-linux-aarch64 bookmarkd-windows-x86_64.exe; do
  [ -f "$out/$asset.manifest.json" ] && continue
  python3 - "$out/$asset" "$version" <<'PY'
import json, pathlib, re, subprocess, sys
binary, version = pathlib.Path(sys.argv[1]), sys.argv[2]
kind = subprocess.check_output(["file", "-b", str(binary)], text=True).strip()
report = {"artifact": binary.name, "version": f"bookmarkd {version}", "file": kind,
          "smoke": "not run: cross-built on macOS arm64 with no machine of this platform available"}
data = binary.read_bytes()
if binary.name.startswith("bookmarkd-linux"):
    glibc = sorted({m.decode() for m in re.findall(rb"GLIBC_2\.\d+", data)}, key=lambda v: int(v.split(".")[1]))
    report["max_glibc"] = glibc[-1] if glibc else None
    assert report["max_glibc"] in (None, "GLIBC_2.28") or int(report["max_glibc"].split(".")[1]) <= 28, report
    assert b"libssl.so" not in data and b"libcrypto.so" not in data and b"libsqlite3.so" not in data
elif binary.name.endswith(".exe"):
    report["dynamic_dependencies"] = "MinGW (GNU) toolchain via cargo-zigbuild; OpenSSL and SQLite statically linked"
else:
    report["dynamic_dependencies"] = subprocess.check_output(["otool", "-L", str(binary)], text=True)
    assert "/opt/homebrew" not in report["dynamic_dependencies"]
binary.with_name(binary.name + ".manifest.json").write_text(json.dumps(report, indent=2) + "\n")
print(f"{binary.name}: {report['smoke']}")
PY
done
cp LICENSE "$out/LICENSE"
cp docs/THIRD_PARTY.md "$out/THIRD_PARTY.md"
(cd "$out" && shasum -a 256 "${assets[@]}" LICENSE THIRD_PARTY.md > SHA256SUMS && shasum -a 256 -c SHA256SUMS >/dev/null)
ls -l "$out"

if [ "$publish" = 0 ]; then
  printf '\nBuilt %s in %s. Run with --publish to tag and release.\n' "$tag" "$out"
  exit 0
fi

step "Publish $tag"
git tag -a "$tag" -m "bookmarkd $tag"
git push origin main "$tag"
gh release create "$tag" --repo "$repo" --verify-tag --generate-notes --prerelease --title "bookmarkd $tag" \
  "${assets[@]/#/$out/}" "$out/LICENSE" "$out/THIRD_PARTY.md" "$out/SHA256SUMS"
check=$(mktemp -d)
gh release download "$tag" --repo "$repo" --dir "$check"
(cd "$check" && shasum -a 256 -c SHA256SUMS)
rm -rf "$check"
printf '\nPublished https://github.com/%s/releases/tag/%s\n' "$repo" "$tag"
