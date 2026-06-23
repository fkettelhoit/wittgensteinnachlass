SANGBLEU_DIR = site/fonts/sangbleu

# Directory holding SangBleu Empire TTFs for the covers tool (Inkscape text-to-path).
# Locally these come from the source font package; CI decompresses WOFF2 into a
# build-only dir and overrides this.
COVERS_FONT_DIR ?= ../../../sangbleu/web files

# Extra flags for the covers tool. CI sets COVERS_FLAGS=--no-font-face so Inkscape
# resolves SangBleu via fontconfig (headless Linux rejects @font-face file:// rules).
COVERS_FLAGS ?=

# Hugo environment for the `quick` and `serve` targets. Defaults to staging (translations
# visible); set ENV=production to check the production layouts (translations hidden).
ENV ?= staging

.PHONY: all production staging quick serve deploy content check-fonts viz covers epub pdf graphics fix translate verify check check-render index clean

# --- Full site builds -------------------------------------------------------
# Identical asset+content pipeline; only the Hugo environment differs. Production
# hides the English translations (hugo.IsProduction); staging keeps them visible.

all: production

production: check-fonts viz covers epub pdf content
	hugo -s site -e production

staging: check-fonts viz covers epub pdf content
	hugo -s site -e staging

# --- Local development ------------------------------------------------------
# Both assume the generated assets (viz, covers, epub, pdf, graphics-cropped) already
# exist from a prior full build — they only rebuild content and re-render Hugo.

# Fast static rebuild into site/public. Staging by default (translations visible);
# `make quick ENV=production` re-renders the production layouts (translations hidden).
quick: content
	hugo -s site -e $(ENV)

# Live preview with auto-reload. Staging by default (translations visible);
# `make serve ENV=production` previews the production layouts (translations hidden).
# Loads the git-ignored repo-root .env (the same file `make index` uses) and maps its
# MEILI_* values onto the HUGO_PARAMS_* names Hugo reads, so the /search page works locally
# without exporting anything by hand. Only the read-only search key reaches Hugo.
serve: content
	@[ -f .env ] && . ./.env; \
		HUGO_PARAMS_MEILIHOST="$$MEILI_HOST" \
		HUGO_PARAMS_MEILISEARCHKEY="$$MEILI_SEARCH_KEY" \
		HUGO_PARAMS_MEILIINDEX="$$MEILI_INDEX_PREFIX" \
		hugo server -s site -e $(ENV)

# --- Deploy -----------------------------------------------------------------
# Production Hugo build + lftp mirror to BunnyCDN. Does NOT regenerate assets —
# run `make production` first if covers, ebooks, PDFs, or visualizations changed.
deploy: content
	hugo -s site -e production
	tools/deploy_site.sh

# --- Build steps ------------------------------------------------------------

# Hugo content (needs md/ + md-en/)
content:
	tools/build_site.sh

check-fonts:
	@if [ ! -f "$(SANGBLEU_DIR)/SangBleuEmpire-Regular-WebS.woff2" ]; then \
		echo "Error: SangBleu fonts not found in $(SANGBLEU_DIR)/"; \
		echo "These are licensed fonts that must be obtained separately."; \
		echo "See $(SANGBLEU_DIR)/README.md for details."; \
		exit 1; \
	fi

# Visualizations (independent, needs only md/)
viz:
	cd tools/visualize && cargo run --release -- --all --all-docs

# Covers (independent, needs only md/)
covers:
	cd tools/covers && cargo run --release -- --all $(COVERS_FLAGS) \
		--font-bold "$(COVERS_FONT_DIR)/SangBleuEmpire-Bold-WebS.ttf" \
		--font-regular "$(COVERS_FONT_DIR)/SangBleuEmpire-Regular-WebS.ttf"

# EPUBs (needs md/ + covers/)
epub: covers
	cd tools/ebooks && cargo run --release -- --all

# PDFs (needs md/ + covers/)
pdf: covers
	cd tools/pdfs && cargo run --release -- --all

# Graphics processing (manual, not part of default build)
graphics:
	cd tools/graphics && ./process_graphics.sh

# --- Translation pipeline ---------------------------------------------------

# Apply mechanical fixes to translations (prefix/suffix changes only, no LLM)
fix:
	cd tools/translate && cargo run --release -- translate --auto-fix-only

# Translate changed remarks (uses git history to detect changes, requires Ollama)
translate:
	cd tools/translate && cargo run --release -- translate --context-ratio=0.2 --num-ctx=4096

verify:
	cd tools/translate && cargo run --release -- verify

# Build-broken gate: fail if English translations are missing, stale (German changed
# since the translation was last committed), or fail quality verification. Read-only,
# no LLM. Requires full git history.
check:
	cd tools/translate && cargo run --release -- check

# --- Misc -------------------------------------------------------------------

# Fail the build on any rendering defect (truncation, emphasis leaking into <math>,
# stray <blockquote>). Reads the built site/public; run after `make quick` / `make
# production` / `make staging`.
check-render:
	python3 tools/verify_render.py

# Push every remark to Meilisearch (full atomic reindex via index swap). Reads
# MEILI_HOST / MEILI_ADMIN_KEY / MEILI_INDEX_PREFIX from the environment or the git-ignored
# repo-root .env (dotenvy finds it by walking up). Add `--dry-run` to preview without a Meili
# instance, or `-- --verify-public ../../site/public` to cross-check anchors against the site.
index:
	cd tools/search-index && cargo run --release

clean:
	rm -rf site/content site/public
