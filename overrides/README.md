# Overrides

Hand-authored, per-remark markdown that replaces generated content so contributor
edits survive regeneration. Overrides are **source** (committed); the parser and the
translate tool read them and never clobber them.

## Layout

- `de/` — German overrides, applied by the parser (`wab-parser`) when generating `md/`.
  Used where the parser can't produce correct output (complex MathML layouts, etc.).
- `en/` — English overrides, applied by the `translate` tool (`tools/translate`) when
  assembling `md-en/`. The override body is used verbatim and is never auto-fixed or
  re-translated.

## Filename convention

`<DocName>_<anchor>.md`, where the remark anchor's segments are joined by `et` (instead
of `+`) and `[N]` page brackets are written as `.N`:

- `Ms-122_82r.2.md` → document `Ms-122`, remark anchor `82r.2`
- `Ms-126_127.2et128.1.md` → document `Ms-126`, remark anchor `127.2+128.1`

The file's contents are the remark body (markdown), without the `###` heading.

## Build gate

Overrides only control *what content is generated* — they are **not** exempt from the
build-broken gate (`make check`). Because overrides flow into the committed `md/` /
`md-en/`, a changed German override whose English override was not updated is caught by
the normal stale check and fails the build, exactly like any other remark. We prefer
over-failing to letting a broken translation through.
