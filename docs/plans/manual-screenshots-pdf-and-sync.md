---
title: Manual — Screenshots, Typeset PDF, and 3-Format Sync Pipeline
status: draft
created: 2026-06-13
---

# Manual: Screenshots, Typeset PDF, and Sync Pipeline

## Goal

1. Add screenshots into the user manual (currently figure-less).
2. Produce a beautifully typeset PDF of the manual (Lotus Agenda User's Guide as
   the visual north star — title page, TOC, running headers, page numbers,
   figures, serif body type).
3. Stand up a `make`-driven pipeline so the **Markdown, HTML, and PDF** versions
   stay in sync from one source.

## Architecture decision (from user)

- **Single source of truth: HTML.** We keep authoring the `docs/*.htm` files and
  *derive* the `.md` and `.pdf` from them. Markdown and PDF become **generated
  artifacts**, not hand-maintained.
- **PDF style: full book reflow into serif prose** with real tables — not the
  monospace/ASCII look of the web page.

### The tension and how we resolve it

The current `.htm` encodes content as **fixed-width ASCII art inside `<pre>`
blocks** (aligned `PURPOSE`/column layouts, `SEE ALSO` blockquotes, anchor names
with spaces). Two things break under the chosen direction:

- A serif PDF cannot reflow monospace-aligned ASCII tables — columns collapse.
- `html→md` of a `<pre>` blob yields an opaque code block, not real structure.

**Resolution:** refactor the HTML source from ASCII-in-`<pre>` into **semantic
HTML** — `<table>` for keybinding/command/column charts, `<dl>` for the
`PURPOSE/USES/HOW IT WORKS` definition blocks, real `<h2>/<h3>` + `<section>`,
`<figure>`/`<img>` for screenshots, `id="kebab"` anchors. A stylesheet keeps the
browser rendering in the existing Lotus terminal aesthetic (monospace, rules,
indented blocks). Because the structure is now semantic, pandoc produces both a
clean Markdown table *and* a properly typeset serif PDF table from the same
source. **This refactor is the bulk of the work.**

Verified locally (smoke test): `pandoc html→pdf` via **xelatex** with
`mainfont=Palatino` + `monofont="IBM Plex Mono"` succeeds; `pandoc html→gfm`
turns `<table>` into a GitHub markdown table. Tooling present: pandoc 3.6.4,
xelatex/lualatex/pdflatex (TeX Live 2023). No browser/weasyprint — so **LaTeX is
the PDF engine**, not HTML-via-Chrome.

## Source / output layout

```
docs/
  src/                      # SOURCE (hand-authored, semantic HTML)
    index.htm
    aglet-manual.htm        # Concepts
    aglet-tui.htm
    aglet-cli.htm
    manual.css              # shared Lotus-style stylesheet (browser look)
  images/                   # screenshots (existing 10 + any new)
  templates/
    pdf.latex               # pandoc LaTeX template (book look)
    metadata.yaml           # title, author, fonts, toc settings
  *.md                      # GENERATED  (do not hand-edit; header banner says so)
  aglet-manual.pdf          # GENERATED  (single combined book PDF)
  Makefile
```

> Moving the source under `docs/src/` keeps generated `.md`/`.pdf` from being
> mistaken for source. If we'd rather not move files, the alternative is to keep
> `.htm` in `docs/` and emit generated `.md`/`.pdf` into `docs/dist/`. Decide at
> kickoff; plan assumes `docs/src/`.

## Makefile targets

- `make html`   — validate source HTML, inline shared CSS / assemble `_site`.
- `make md`     — `pandoc src/X.htm -t gfm` → `docs/X.md`, prepend a
                  "generated — edit the .htm" banner; rewrite `.htm` links to `.md`.
- `make pdf`    — concatenate the three docs (Concepts → TUI → CLI) into one
                  **`aglet-manual.pdf`** via `pandoc ... --pdf-engine=xelatex`
                  with `templates/pdf.latex` + `metadata.yaml` (title page, TOC,
                  `\chapter` per doc, running headers, figures).
- `make all`    — `html md pdf`.
- `make check`  — regenerate `md`/`pdf` and `git diff --exit-code` so committed
                  artifacts can't drift from the HTML source (CI guard).
- `make clean`  — remove generated outputs.

## PDF typesetting design

- Engine: **xelatex** (system fonts). Body **Palatino** (or Charter/Baskerville —
  all installed), mono **IBM Plex Mono**, sans heading face optional.
- One combined book: title page, auto TOC, three chapters, `\fancyhdr` running
  headers (doc title left / section right), page numbers, figure captions, link
  colors. Start from a custom `templates/pdf.latex` (Eisvogel-style) tuned to a
  restrained, print-manual look.
- Tables: `longtable`/`booktabs` (pandoc default) for keybinding & command
  charts. Code/terminal samples stay monospace in a light-ruled box.

## Screenshots (gap audit → confirm → wire in)

10 screenshots already exist in `docs/images/` (used only by root README today).
I will:

1. Audit each manual section for "needs a figure," map the existing 10 to
   sections, and flag sections with no good visual (candidates for new captures,
   e.g. specific TUI modes/panels).
2. **Post a placement proposal table for your approval** (section → image →
   caption) *before* wiring anything in.
3. On approval, add `<figure><img><figcaption>` into the semantic HTML so the
   figures flow to all three outputs. New captures (if any) follow the existing
   filename/style convention.

## CI / Pages

- Extend `.github/workflows/pages.yml`: install pandoc + a minimal TeX Live,
  run `make all`, publish `_site` **plus `aglet-manual.pdf`**, and add a
  "Download PDF" link on the home page. Update the `paths:` trigger to the new
  `docs/src/**`, `docs/images/**`, `docs/templates/**`, `Makefile`.
- Add a CI `make check` (or commit generated artifacts) so `.md`/`.pdf` never
  drift from the HTML source.

## Phases

1. **Pipeline skeleton** — create `Makefile`, `templates/pdf.latex`,
   `metadata.yaml`, `manual.css`; prove `make all` on the *current* content
   (even if PDF still shows pre-blocks). Lands the build before the rewrite.
2. **Semantic HTML refactor** — convert `index` + the 3 docs from ASCII-`<pre>`
   to semantic markup; restyle via `manual.css` to preserve the browser look.
   Re-run `make all`; tune the PDF template to a clean book layout.
3. **Screenshots** — gap audit, get approval on placement, wire `<figure>`s in.
4. **CI** — update Pages workflow (build + publish PDF + `make check` guard).
5. **Docs/cleanup** — note the source-of-truth rule in `docs/README.md`; add
   "generated, do not edit" banners to `.md`; remove the old hand-maintained
   `.md` duplication.

## Risks / open items

- **Refactor size**: ~2,000 lines of `<pre>` HTML across 3 docs become semantic.
  Mechanical but substantial; Phase 2 is the heavy lift.
- **PDF look iteration**: matching the Lotus guide's polish is a tuning loop on
  `pdf.latex` (margins, fonts, header rules, figure sizing).
- **CI TeX install** time/size — use a slim TeX Live set or a pandoc+latex
  container action.
- **Decision needed at kickoff**: `docs/src/` move vs `docs/dist/` outputs; final
  body font; one combined PDF vs per-doc PDFs (plan assumes combined).
