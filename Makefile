SANGBLEU_DIR = site/fonts/sangbleu

# Directory holding SangBleu Empire TTFs for the covers tool (Inkscape text-to-path).
# Locally these come from the source font package; CI decompresses WOFF2 into a
# build-only dir and overrides this.
COVERS_FONT_DIR ?= ../../../sangbleu/web files

# Extra flags for the covers tool. CI sets COVERS_FLAGS=--no-font-face so Inkscape
# resolves SangBleu via fontconfig (headless Linux rejects @font-face file:// rules).
COVERS_FLAGS ?=

.PHONY: all site quick serve deploy fix translate verify check viz covers epub pdf content hugo graphics index clean check-fonts

all: site

site: check-fonts viz covers epub pdf content hugo

quick: content hugo

serve: content
	hugo server -s site

# Deploy to BunnyCDN (rebuild content + production Hugo + lftp mirror)
# Run `make site` first if you need to regenerate assets (viz, covers, epub, pdf)
deploy: content
	hugo -s site -e production
	tools/deploy_site.sh

check-fonts:
	@if [ ! -f "$(SANGBLEU_DIR)/SangBleuEmpire-Regular-WebS.woff2" ]; then \
		echo "Error: SangBleu fonts not found in $(SANGBLEU_DIR)/"; \
		echo "These are licensed fonts that must be obtained separately."; \
		echo "See $(SANGBLEU_DIR)/README.md for details."; \
		exit 1; \
	fi

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

# Hugo content (needs md/ + md-en/)
content:
	tools/build_site.sh

# Hugo build (needs content + all assets for mounts)
hugo: content viz covers epub pdf
	hugo -s site

# Push every remark to Meilisearch (full atomic reindex via index swap). Reads
# MEILI_HOST / MEILI_ADMIN_KEY / MEILI_INDEX_PREFIX from the environment or from a
# git-ignored tools/search-index/.env. Add `--dry-run` to preview without a Meili instance,
# or `-- --verify-public ../../site/public` to cross-check anchors against the rendered site.
index:
	cd tools/search-index && cargo run --release

# Graphics processing (manual, not part of default build)
graphics:
	cd tools/graphics && ./process_graphics.sh

clean:
	rm -rf site/content site/public
