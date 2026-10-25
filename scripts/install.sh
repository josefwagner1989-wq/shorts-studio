#!/usr/bin/env sh
# Installs Shorts Studio from a .flatpak bundle.
#   ./install.sh                       uses shorts-studio.flatpak next to this script
#   ./install.sh path/to/bundle.flatpak
#   ./install.sh --latest              downloads the newest release (needs `gh auth login`
#                                      while the repository is private)
set -eu
APP=io.github.shortsstudio.ShortsStudio
REPO=josefwagner1989-wq/shorts-studio
here=$(cd "$(dirname "$0")" && pwd)

if ! command -v flatpak >/dev/null 2>&1; then
  echo "Flatpak is not installed. See https://flatpak.org/setup/ for your distribution, then run this again." >&2
  exit 1
fi

bundle="${1:-$here/shorts-studio.flatpak}"
if [ "$bundle" = "--latest" ]; then
  command -v gh >/dev/null 2>&1 || { echo "Install the GitHub CLI (gh) and run 'gh auth login' first." >&2; exit 1; }
  tmp=$(mktemp -d)
  gh release download --repo "$REPO" --pattern '*.flatpak' --dir "$tmp"
  bundle=$(ls "$tmp"/*.flatpak | head -1)
fi
[ -f "$bundle" ] || { echo "Bundle not found: $bundle" >&2; exit 1; }

echo "Adding Flathub (for the GNOME runtime) if needed…"
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo

echo "Installing $bundle …"
flatpak install --user -y --noninteractive "$bundle"

echo
echo "Done. Start Shorts Studio from your app menu, or run: flatpak run $APP"
