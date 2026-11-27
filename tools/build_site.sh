#!/bin/bash
# Transforms parser markdown output into Hugo content pages.
# Reads from md/ and md-en/, writes to site/content/.
#
# The pages are built in a temporary folder and then synced into site/content/, which
# only rewrites the files that changed and removes the ones that are gone: a running
# `hugo server` keeps serving and re-renders just those pages (it loses its pages if
# site/content/ is deleted under it).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/../md"
EN_DIR="$SCRIPT_DIR/../md-en"
SITE_DIR="$SCRIPT_DIR/../site"
SIMILAR_TSV="$SCRIPT_DIR/../similar/similar.tsv"
SITE_CONTENT_DIR="$SITE_DIR/content"

rm -rf "$SITE_DIR/public"

# The new content, synced into site/content/ at the end.
CONTENT_DIR="$(mktemp -d)"
# Document copies with links to other versions of each remark (see add_versions.py).
VERSIONS_DIR="$(mktemp -d)"
trap 'rm -rf "$VERSIONS_DIR" "$CONTENT_DIR"' EXIT
python3 "$SCRIPT_DIR/add_versions.py" "$OUTPUT_DIR" "$SIMILAR_TSV" "$VERSIONS_DIR"

# The pages are independent of each other, so they are made in parallel (german_page and
# bilingual_page, one process per page, as many at a time as there are cores).
JOBS="$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo 4)"
export SCRIPT_DIR OUTPUT_DIR EN_DIR CONTENT_DIR VERSIONS_DIR
run_parallel() { # function, then the files (NUL-separated on stdin)
  xargs -0 -n1 -P "$JOBS" bash -c 'set -euo pipefail; "$0" "$1"' "$1"
}

german_page() {
  src="$1"
  filename="$(basename "$src")"

  # Skip the full index — handled separately below
  if [ "$filename" = "all.md" ]; then
    return
  fi

  # Handle index.md separately — it becomes _index.md (Hugo's homepage content), once
  # for the German home page (/) and once for the German/English one (/en/); see
  # site/layouts/partials/home.html.
  if [ "$filename" = "index.md" ]; then
    mkdir -p "$CONTENT_DIR/en"
    for home in "$CONTENT_DIR/_index.md" "$CONTENT_DIR/en/_index.md"; do
      {
        echo "---"
        echo "title: \"Wittgenstein\u2019s Writings\""
        echo "---"
        echo ""
        # Rewrite relative markdown links to Hugo permalinks:
        # (Ms-175.md) → (/ms-175/)
        tail -n +2 "$src" \
          | perl -pe 's/\(([A-Za-z]+-[\w-]+)\.md\)/"(\/" . lc($1) . "\/)"/ge'
      } > "$home"
    done
    return
  fi

  # Extract title from first line: "# Ms-116" → "Ms-116"
  title="$(head -1 "$src" | sed 's/^# //')"

  # Determine kind and numeric weight for sorting
  # Use filename prefix to detect work files (W-*.md)
  kind="$(echo "$filename" | cut -d'-' -f1)"
  if [ "$kind" = "W" ]; then
    # Work files: use weight 2000+ to sort after documents
    weight="$((2000 + $(echo "$filename" | cksum | cut -d' ' -f1) % 900))"
    doctype="Work"
  else
    rest="${title#*-}"    # "116" or "201a1"
    # Extract leading digits for weight; Ms gets 0-prefix, Ts gets 1-prefix
    num="$(echo "$rest" | grep -o '^[0-9]*')"
    if [ "$kind" = "Ms" ]; then
      weight="$num"
    else
      weight="$((1000 + num))"
    fi
    doctype="$kind"
  fi

  # Check if an English translation exists for this document
  has_translation="false"
  if [ -f "$EN_DIR/$filename" ]; then
    has_translation="true"
  fi

  {
    echo "---"
    echo "title: \"$title\""
    echo "weight: $weight"
    echo "doctype: $doctype"
    if [ "$has_translation" = "true" ]; then
      echo "translation: true"
    fi
    echo "---"
    echo ""
    # Neutralize Markdown accidentally triggered by transcription notation.
    # Documents are read from their copy with version links, where there is one.
    body_src="$src"
    if [ -f "$VERSIONS_DIR/$filename" ]; then
      body_src="$VERSIONS_DIR/$filename"
    fi
    python3 "$SCRIPT_DIR/sanitize_content.py" < "$body_src"
  } > "$CONTENT_DIR/$filename"
}
export -f german_page
printf '%s\0' "$OUTPUT_DIR"/*.md | run_parallel german_page

# Create full text index at /all
ALL_SRC="$OUTPUT_DIR/all.md"
if [ -f "$ALL_SRC" ]; then
  mkdir -p "$CONTENT_DIR/all"
  {
    echo "---"
    echo "title: \"Wittgenstein\u2019s Writings\""
    echo "layout: all"
    echo "---"
    echo ""
    # Rewrite markdown links: (Ms-175.md) → (/ms-175/)
    tail -n +2 "$ALL_SRC" \
      | perl -pe 's/\(([A-Za-z]+-[\w-]+)\.md\)/"(\/" . lc($1) . "\/)"/ge' \
      | python3 "$SCRIPT_DIR/sanitize_content.py"
  } > "$CONTENT_DIR/all/_index.md"
fi

# Create the Nachlass Map at /map (every remark as a block; see tools/map/README.md).
# It takes a while, so it is kept in site/.map-cache/ (git-ignored) and only made anew
# when its inputs changed: md/, similar.tsv, or the scripts that make it.
MAP_CACHE="$SITE_DIR/.map-cache"
map_inputs="$(cat "$OUTPUT_DIR"/*.md "$SIMILAR_TSV" "$SCRIPT_DIR"/map/*.py "$SCRIPT_DIR/add_versions.py" | shasum | cut -d' ' -f1)"
if [ -d "$MAP_CACHE/map" ] && [ "$(cat "$MAP_CACHE/inputs" 2>/dev/null)" = "$map_inputs" ]; then
  echo "Nachlass Map: unchanged."
else
  rm -rf "$MAP_CACHE"
  mkdir -p "$MAP_CACHE"
  python3 "$SCRIPT_DIR/map/map.py" "$OUTPUT_DIR" "$SIMILAR_TSV" "$MAP_CACHE"
  echo "$map_inputs" > "$MAP_CACHE/inputs"
fi
cp -R "$MAP_CACHE/map" "$CONTENT_DIR/map"

# Create the page explaining the works' sigils at /sigils (from the parser's titles)
TITLES_SRC="$OUTPUT_DIR/work_titles.tsv"
if [ -f "$TITLES_SRC" ]; then
  mkdir -p "$CONTENT_DIR/sigils"
  python3 "$SCRIPT_DIR/sigils_page.py" "$TITLES_SRC" > "$CONTENT_DIR/sigils/_index.md"
fi

# Create the About page at /about
ABOUT_SRC="$SCRIPT_DIR/../about.md"
if [ -f "$ABOUT_SRC" ]; then
  mkdir -p "$CONTENT_DIR/about"
  {
    echo "---"
    echo "title: \"What Are Wittgenstein’s Writings?\""
    echo "layout: about"
    echo "---"
    echo ""
    # Drop the leading "# About" (title comes from the layout's <h1>).
    tail -n +2 "$ABOUT_SRC"
  } > "$CONTENT_DIR/about/_index.md"
fi

# Generate bilingual pages for documents with English translations
en_count=0
if [ -d "$EN_DIR" ]; then
  EN_CONTENT_DIR="$CONTENT_DIR/en"
  mkdir -p "$EN_CONTENT_DIR"
  export EN_CONTENT_DIR

  bilingual_page() {
    en_src="$1"
    [ -f "$en_src" ] || return 0
    filename="$(basename "$en_src")"
    de_src="$OUTPUT_DIR/$filename"
    [ -f "$de_src" ] || return 0
    # The German side comes from the copy with version links, as on the German page.
    de_body_src="$de_src"
    if [ -f "$VERSIONS_DIR/$filename" ]; then
      de_body_src="$VERSIONS_DIR/$filename"
    fi

    title="$(head -1 "$de_src" | sed 's/^# //')"
    entitle="$(head -1 "$en_src" | sed 's/^# //')"

    kind="$(echo "$filename" | cut -d'-' -f1)"
    if [ "$kind" = "W" ]; then
      weight="$((2000 + $(echo "$filename" | cksum | cut -d' ' -f1) % 900))"
      doctype="Work"
    else
      rest="${title#*-}"
      num="$(echo "$rest" | grep -o '^[0-9]*')"
      if [ "$kind" = "Ms" ]; then
        weight="$num"
      else
        weight="$((1000 + num))"
      fi
      doctype="$kind"
    fi

    {
      echo "---"
      echo "title: \"$title\""
      echo "entitle: \"$entitle\""
      echo "weight: $weight"
      echo "doctype: $doctype"
      echo "layout: bilingual"
      echo "---"
      echo ""
      python3 "$SCRIPT_DIR/merge_bilingual.py" "$de_body_src" "$en_src" \
        | python3 "$SCRIPT_DIR/sanitize_content.py"
    } > "$EN_CONTENT_DIR/$filename"
  }
  export -f bilingual_page
  printf '%s\0' "$EN_DIR"/*.md | run_parallel bilingual_page
  en_count="$(find "$EN_CONTENT_DIR" -name '*.md' ! -name '_index.md' | wc -l | tr -d ' ')"
fi

# Only the files whose content changed are rewritten (by checksum, not time).
mkdir -p "$SITE_CONTENT_DIR"
changed="$(rsync -r --checksum --delete --out-format='%n' "$CONTENT_DIR/" "$SITE_CONTENT_DIR/" | grep -vc '/$' || true)"

echo "Built $(ls "$CONTENT_DIR"/*.md | wc -l | tr -d ' ') content files ($en_count bilingual), $changed changed."
