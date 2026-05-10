# aglet

aglet is a free-form personal information manager (PIM) modeled after [Lotus
Agenda](https://en.wikipedia.org/wiki/Lotus_Agenda), re-imagined as a modern
TUI.

It allows you to input unstructured notes and to-dos, then categorize them
either manually or with automatic rules based on the text (in aglet there is
experimental support for LLM-based categorization).

Currently I use this software to manage my GTD-style todo list, track basic
financial budgets and recurring bills, and track motorcycle maintenance. It
also acts as a backing store for my own personal knowledge base db, inspired by
Andrej Karpathy's [LLM
wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)
concept, and Garry Tan's [gbrain](https://github.com/garrytan/gbrain).


## What was Lotus Agenda?

1. Kapor/Belove/Kaplan ACM paper (1990):
  - PDF: https://dl.acm.org/doi/pdf/10.1145/79204.79212
  - DOI page: https://dl.acm.org/doi/10.1145/79204.79212

2. Fallows Atlantic article (May 1992):
  - Title: "Hidden Powers: Agenda Can Organize Your Life Better Than Ordinary Programs"
  - URL: https://cdn.theatlantic.com/media/archives/1992/05/269-5/132676110.pdf
  - Bonus — the draft text (95% similar per Bob Newell's site): https://www.bobnewell.net/agenda/atlantic.txt.html

## What is aglet?

aglet is a Rust TUI for managing items, categories, views, and item dependency
links. It also includes a CLI, primarily for ease of use by LLM coding agents.

It currently implements the basic feature set of Lotus Agenda. aglet's
keybindings are different, but otherwise pretty much all of the historical
Agenda articles and tutorials you can find online can be accomplished within
aglet.

Of the original Lotus Agenda, Wikipedia notes "Its flexibility proved to be its
weakness. New users confronted with so much flexibility were often overpowered
by the steep learning curve required to use the program."

But aglet is experimental software built by and for a single person (me). Part
of the motivation is to revisit some of these concepts and see if there is
anything to be learned, are there concepts or ideas ready to be revived 35+
years later.


## Quick Start

```bash
cargo run --bin aglet -- --db aglet-features.ag
cargo run --bin aglet -- --db aglet-features.ag list --view "All Items"
```

Running `aglet` without a subcommand opens the TUI. Use `aglet list` for
non-interactive list output.

## Standalone Binary Build

Build a release-mode standalone package for the current platform:

```bash
scripts/build-standalone-aglet.sh
```

The package is written to `dist/aglet-<version>-<platform>.tar.gz` with a
matching SHA-256 checksum file. GitHub Actions runs the same procedure on every
pull request, and publishes the packaged binaries to the `aglet-latest`
prerelease when changes land on `main` or `master`.

## References

- Bob Newell's Agenda Page. — Archive of original documentation, FAQs, macros,
  and add-ons. <https://www.bobnewell.net/agenda.html>
- Kapor, M., Belove, E., Kaplan, J. (1990). "AGENDA: A Personal Information
  Manager." *Communications of the ACM*, 33(7). — The foundational paper
  describing the item/category data model.
  [PDF](https://dl.acm.org/doi/pdf/10.1145/79204.79212) ·
  [DOI](https://dl.acm.org/doi/10.1145/79204.79212)
- Fallows, J. (May 1992). "Hidden Powers: Agenda Can Organize Your Life Better
  Than Ordinary Programs." *The Atlantic Monthly*. — The best plain-English
  description of what Agenda does and why it matters.
  [PDF](https://cdn.theatlantic.com/media/archives/1992/05/269-5/132676110.pdf) ·
  [Draft text mirror](https://www.bobnewell.net/agenda/atlantic.txt.html)
- Ormandy, T. (2020). "Lotus Agenda." — Modern hands-on exploration with
  working examples. <https://lock.cmpxchg8b.com/lotusagenda.html>
- Rosenberg, S. (2007). *Dreaming in Code*. — Documents Chandler's failure to
  rebuild Agenda. 
