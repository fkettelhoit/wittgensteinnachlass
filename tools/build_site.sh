#!/bin/bash
# Transforms parser markdown output into Hugo content pages.
# Reads from md/ and md-en/, writes to site/content/.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/../md"
EN_DIR="$SCRIPT_DIR/../md-en"
SITE_DIR="$SCRIPT_DIR/../site"
CONTENT_DIR="$SITE_DIR/content"

rm -rf "$CONTENT_DIR" "$SITE_DIR/public"
mkdir -p "$CONTENT_DIR"

for src in "$OUTPUT_DIR"/*.md; do
  filename="$(basename "$src")"

  # Skip browse pages — handled separately below
  if [ "$filename" = "docs-by-date.md" ] || [ "$filename" = "docs-by-name.md" ] || [ "$filename" = "all.md" ]; then
    continue
  fi

  # Handle index.md separately — it becomes _index.md (Hugo's homepage content)
  if [ "$filename" = "index.md" ]; then
    {
      echo "---"
      echo "title: \"Wittgenstein\u2019s Writings\""
      echo "---"
      echo ""
      # Rewrite relative markdown links to Hugo permalinks:
      # (Ms-175.md) → (/ms-175/)
      tail -n +2 "$src" \
        | perl -pe 's/\(([A-Za-z]+-[\w-]+)\.md\)/"(\/" . lc($1) . "\/)"/ge'
    } > "$CONTENT_DIR/_index.md"
    continue
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
    cat "$src"
  } > "$CONTENT_DIR/$filename"
done

# Create document browsing pages
for page in docs-by-date docs-by-name; do
  src="$OUTPUT_DIR/${page}.md"
  if [ -f "$src" ]; then
    mkdir -p "$CONTENT_DIR/$page"
    {
      echo "---"
      echo "title: \"Wittgenstein\u2019s Writings\""
      echo "layout: gallery"
      echo "---"
      echo ""
      cat "$src"
    } > "$CONTENT_DIR/$page/_index.md"
  fi
done

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
      | perl -pe 's/\(([A-Za-z]+-[\w-]+)\.md\)/"(\/" . lc($1) . "\/)"/ge'
  } > "$CONTENT_DIR/all/_index.md"
fi

# Create the About page at /about
ABOUT_SRC="$SCRIPT_DIR/../about.md"
if [ -f "$ABOUT_SRC" ]; then
  mkdir -p "$CONTENT_DIR/about"
  {
    echo "---"
    echo "title: \"About Wittgenstein’s (Late) Writings\""
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

  cat > "$EN_CONTENT_DIR/_index.md" <<'ENINDEX'
---
title: "Translations"
---
ENINDEX

  for en_src in "$EN_DIR"/*.md; do
    [ -f "$en_src" ] || continue
    filename="$(basename "$en_src")"
    de_src="$OUTPUT_DIR/$filename"
    [ -f "$de_src" ] || continue

    title="$(head -1 "$de_src" | sed 's/^# //')"

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
      echo "weight: $weight"
      echo "doctype: $doctype"
      echo "layout: bilingual"
      echo "---"
      echo ""
      python3 "$SCRIPT_DIR/merge_bilingual.py" "$de_src" "$en_src"
    } > "$EN_CONTENT_DIR/$filename"

    en_count=$((en_count + 1))
  done
fi

echo "Built $(ls "$CONTENT_DIR"/*.md | wc -l | tr -d ' ') content files ($en_count bilingual)."
