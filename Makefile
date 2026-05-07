SANGBLEU_DIR = site/fonts/sangbleu

.PHONY: all site quick serve deploy translate viz covers epub pdf content hugo graphics clean check-fonts

all: site

site: check-fonts viz covers epub pdf content hugo

quick: content hugo

serve: content
	hugo server -s site

# Deploy to BunnyCDN (full build + production Hugo rebuild + lftp mirror)
deploy: translate site
	hugo -s site -e production
	./deploy.sh

check-fonts:
	@if [ ! -f "$(SANGBLEU_DIR)/SangBleuEmpire-Regular-WebS.woff2" ]; then \
		echo "Error: SangBleu fonts not found in $(SANGBLEU_DIR)/"; \
		echo "These are licensed fonts that must be obtained separately."; \
		echo "See $(SANGBLEU_DIR)/README.md for details."; \
		exit 1; \
	fi

# Translate changed remarks (uses git history to detect changes, requires Ollama)
translate:
	cd tools/translate && cargo run --release -- translate --glossary ../../glossary.md --context-ratio=0.2 --num-ctx=4096

verify:
	cd tools/translate && cargo run --release -- verify

# Visualizations (independent, needs only md/)
viz:
	cd tools/visualize && cargo run --release -- --all

# Covers (independent, needs only md/)
covers:
	cd tools/covers && cargo run --release -- --all

# EPUBs (needs md/ + covers/)
epub: covers
	cd tools/ebooks && cargo run --release -- --all

# PDFs (needs md/ + covers/)
pdf: covers
	cd tools/pdfs && cargo run --release -- --all

# Hugo content (needs md/ + md-en/)
content:
	tools/content.sh

# Hugo build (needs content + all assets for mounts)
hugo: content viz covers epub pdf
	hugo -s site

# Graphics processing (manual, not part of default build)
graphics:
	cd tools/graphics && ./process_graphics.sh

clean:
	rm -rf site/content site/public
