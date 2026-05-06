# translate

Translates German Nachlass markdown files to English using a local LLM (Ollama) or the DeepL API. Supports glossary-based translation, automatic verification, and adaptive batching.

## Subcommands

### translate

Translate document files and assemble works using Ollama.

```
cargo run --release -- translate --glossary ../../../glossary.md
```

Translates all untranslated documents listed in `index.md`, verifies each remark inline, and assembles work files from completed translations. Resumes from partial files if interrupted. Automatically detects remarks that changed in the German source since the last translation (using git history) and re-translates only those.

Options:

- `--input` -- German markdown directory (default: `../../md`)
- `--output` -- English output directory (default: `../../md-en`)
- `--model` -- Ollama model name (default: `translategemma:27b`)
- `--ollama-url` -- Ollama API base URL (default: `http://localhost:11434`)
- `--glossary` -- path to glossary file (default: `glossary.md`)
- `--no-glossary` -- proceed without a glossary
- `--num-ctx` -- Ollama context window in tokens (default: `8192`)
- `--context-ratio` -- fraction of context window for history vs. new remarks (default: `0.5`)
- `--emphasis-tolerance` -- allowed underscore mismatch before flagging (default: `4`)
- `--no-verify` -- skip verification and fixing of existing translations (still detects changed remarks)
- `--verbose` -- log prompts and translations to stderr

### verify

Check translation quality by comparing German and English files.

```
cargo run --release -- verify
```

Reports structural issues (missing math/HTML/images), emphasis mismatches, straight quotes, truncation, and untranslated text.

### fix-deepl

Fix broken remarks using the DeepL API. Runs independently of the Ollama-based translate command.

```
cargo run --release -- fix-deepl --glossary ../../../glossary.md
```

Verifies all translations, re-translates broken remarks via DeepL, and logs every attempt to `deepl-remarks.md` for review. Requires a DeepL API key via `--deepl-key` or the `DEEPL_API_KEY` environment variable (can be set in a `.env` file).

### migrate

Copy updated headings from German originals to English translations without re-translating.

```
cargo run --release -- migrate
```

Matches remarks by position and anchor ID, replacing only the heading line while preserving the translated body.

## Glossary

The tool uses a glossary file to ensure consistent translation of philosophical terminology. Unambiguous terms (single translation) are enforced directly; ambiguous terms (multiple options like `Satz = sentence / proposition`) are passed as context hints. The glossary's general translation principles are included in the system prompt.

## Sibling document reuse

When translating a document variant (e.g., `Ts-227b.md`), the tool checks for an already-translated sibling (`Ts-227a.md`) and reuses translations for remarks with matching content (>85% word overlap), only translating the differences.

## Dependencies

- [Ollama](https://ollama.com/) running locally with a translation model (for `translate`)
- DeepL API key (for `fix-deepl`, free tier available)
