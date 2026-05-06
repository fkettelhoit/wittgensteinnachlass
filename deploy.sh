#!/bin/bash
# Deploys site/public/ to BunnyCDN storage via lftp.
# Credentials are read from ~/.netrc.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SITE_DIR="$SCRIPT_DIR/site"
LOCAL_DIR="$SITE_DIR/public/"

# Full build (generate all assets + Hugo content)
echo "Running full site build..."
make -C "$SCRIPT_DIR" site

# Rebuild site in production mode (hides draft translation links)
echo "Building site in production mode..."
hugo -s "$SITE_DIR" -e production

# Mirror may exit non-zero due to BunnyCDN rmdir failures (expected)
lftp storage.bunnycdn.com -e "
  mirror --reverse --delete --verbose --ignore-time --parallel=5 --exclude '\.DS_Store$' $LOCAL_DIR .
  bye
" || true

# BunnyCDN's FTP rmdir doesn't work on empty directories left behind by
# mirror --delete. Clean them up by listing remote dirs and removing any
# that don't exist locally.
echo "Cleaning stale remote directories..."
remote_dirs=$(lftp storage.bunnycdn.com -e "cls -1 --sort=name; bye" 2>/dev/null) || true
echo "$remote_dirs" | while read -r remote; do
  remote="${remote%/}"
  [ -z "$remote" ] && continue
  if [ ! -e "$LOCAL_DIR$remote" ]; then
    echo "Removing stale: $remote"
    lftp storage.bunnycdn.com -e "rm -rf \"$remote\"; bye" 2>/dev/null || true
  fi
done
