# Aglet manual build — HTML is the single source of truth.
#
#   src/*.htm  (hand-authored, semantic)  ->  *.md  and  aglet-manual.pdf
#
# Edit only files under src/.  The .md files and the PDF are generated; never
# hand-edit them.  `make check` fails if they have drifted from the source.

PANDOC      ?= pandoc
PDF_ENGINE  ?= xelatex
BUILD_DATE  := $(shell date +%Y-%m-%d)

# Source docs (without extension). index is the home page; the three content
# docs are concatenated, in order, into the combined PDF book.
DOCS        := index aglet-manual aglet-tui aglet-cli
PDF_DOCS    := aglet-manual aglet-tui aglet-cli

SRC         := $(addprefix src/,$(addsuffix .htm,$(DOCS)))
MD          := $(addsuffix .md,$(DOCS))
PDF         := aglet-manual.pdf
SITE        := _site

GEN_BANNER  := <!-- GENERATED from docs/src/$*.htm — DO NOT EDIT. Run `make md` in docs/. -->

.PHONY: all html md pdf check clean
all: html md pdf

## md: derive GitHub-flavored Markdown from each HTML source.
md: $(MD)
%.md: src/%.htm
	@$(PANDOC) $< -f html -t gfm --wrap=preserve -o $@.tmp
	@# .htm cross-links -> .md, then prepend the "generated" banner.
	@sed -e 's/\.htm)/.md)/g' -e 's/\.htm#/.md#/g' $@.tmp > $@.tmp2
	@printf '%s\n\n' "$(GEN_BANNER)" | cat - $@.tmp2 > $@
	@rm -f $@.tmp $@.tmp2
	@echo "  MD   $@"

## pdf: typeset the three content docs into one book via xelatex.
pdf: $(PDF)
$(PDF): $(addprefix src/,$(addsuffix .htm,$(PDF_DOCS))) templates/metadata.yaml templates/header.tex
	@$(PANDOC) $(addprefix src/,$(addsuffix .htm,$(PDF_DOCS))) \
	  -f html \
	  --metadata-file=templates/metadata.yaml \
	  --metadata title="Aglet Manual" \
	  --metadata date="$(BUILD_DATE)" \
	  --include-in-header=templates/header.tex \
	  --pdf-engine=$(PDF_ENGINE) \
	  --resource-path=.:src \
	  -o $@
	@echo "  PDF  $@"

## html: stage a flat site (htm + css + images) for preview / Pages.
html:
	@rm -rf $(SITE) && mkdir -p $(SITE)
	@cp src/index.htm $(SITE)/index.html
	@cp src/aglet-manual.htm src/aglet-tui.htm src/aglet-cli.htm $(SITE)/
	@[ -f src/manual.css ] && cp src/manual.css $(SITE)/ || true
	@cp -R images $(SITE)/images
	@echo "  HTML $(SITE)/"

## check: regenerate md+pdf and fail if anything drifted from source (CI guard).
check: md pdf
	@git diff --exit-code -- $(MD) $(PDF) \
	  || { echo "ERROR: generated docs differ from source. Run 'make all' and commit."; exit 1; }
	@echo "  OK   md/pdf in sync with src/"

clean:
	@rm -rf $(SITE) $(MD) $(PDF) *.tmp *.tmp2
	@echo "  CLEAN"
