#!/bin/bash
# Deploys site/public/ to BunnyCDN storage via lftp.
# Credentials are read from ~/.netrc; the storage host can be overridden via
# BUNNY_STORAGE_HOST (defaults to storage.bunnycdn.com for local use).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LOCAL_DIR="$SCRIPT_DIR/../site/public/"
HOST="${BUNNY_STORAGE_HOST:-storage.bunnycdn.com}"

# Mirror may exit non-zero due to BunnyCDN rmdir failures (expected).
# fonts/sangbleu/ holds licensed fonts that live only on the CDN and are not part
# of the build output, so exclude them: never re-upload and never delete them.
lftp "$HOST" -e "
  mirror --reverse --delete --verbose --ignore-time --parallel=5 --exclude '\.DS_Store$' --exclude 'fonts/sangbleu/' $LOCAL_DIR .
  bye
" || true

# BunnyCDN's FTP rmdir doesn't work on empty directories left behind by
# mirror --delete. Clean them up by listing remote dirs and removing any
# that don't exist locally.
echo "Cleaning stale remote directories..."
remote_dirs=$(lftp "$HOST" -e "cls -1 --sort=name; bye" 2>/dev/null) || true
echo "$remote_dirs" | while read -r remote; do
  remote="${remote%/}"
  [ -z "$remote" ] && continue
  # Never touch the licensed fonts directory (lives only on the CDN).
  [ "$remote" = "fonts" ] && continue
  if [ ! -e "$LOCAL_DIR$remote" ]; then
    echo "Removing stale: $remote"
    lftp "$HOST" -e "rm -rf \"$remote\"; bye" 2>/dev/null || true
  fi
done
