---
title: TUI Workflow Analysis Summary
status: draft
created: 2026-03-15
---

# TUI Workflow Analysis: Summary

**Date**: 2026-02-20
**Status**: Analysis complete
**Full details**: See `docs/specs/proposals/tui-add-workflow-analysis.md`

## Key Findings

### 1. The Core Problem: Dissonant User Journeys

When **adding** an item:
- User presses 'n' → enters Mode::AddInput (bare text box)
- Types text → presses Enter → item created with only text
- Must press 'm' (separate mode) to add note → Mode::NoteEdit
- Must press 'a' (separate mode) to assign categories → Mode::ItemAssignPicker
- **Total**: 3 modes, 7+ keystrokes, non-discoverable pattern

When **editing** an item:
- User presses Enter/e → Mode::ItemEdit (rich form popup)
- Sees text, note, categories button, save/cancel buttons all on one screen
- Can Tab between fields, toggle categories inline, see everything at once
- Presses Enter on Save → item saved
- **Total**: 1 mode, 5 keystrokes, discoverable pattern

**Disconnect**: Users recognize that edit is powerful and ask (feature request #1): *"Why can't I add items the same way?"*

### 2. Fallows Article Insight: Lost Opportunity

The 1992 Lotus Agenda review emphasizes **automatic intelligent assignment**:

> "Any item including the words 'Call Mom' can automatically be sent to the 'high priority' category... If you type in 'See Sue next Wednesday about Hmong families in Fresno,' the item can automatically appear in views displaying 'topics to discuss with Sue,' 'events next week,' 'immigration from Asia,' 'California sociology'..."

Aglet implements this via `Section.on_insert_assign` rules, but **users can't see or control them at creation time**. They silently apply after the item is created. The proposed InputPanel would:
- Show the automatic assignments in a preview footer before save
- Let user add/remove categories before committing
- Provide visual feedback on what will happen

### 3. The General Solution: Input Panel Abstraction

Instead of:
- Mode::AddInput (text only)
- Mode::ItemEdit (rich form)
- Mode::ViewCreateName (name only)
- Mode::ViewRename
- Mode::CategoryCreate
- Mode::CategoryRename
- etc.

Propose one abstraction:

```
Mode::InputPanel {
  kind: AddItem | EditItem | NameInput,
  text: TextBuffer,
  note: TextBuffer,
  categories: HashSet<CategoryId>,
  focus: Text | Note | CategoriesButton | SaveButton | CancelButton,
  ... overlay/preview state
}
```

**Benefits**:
- Consistent interface across add, edit, name workflows
- Reuses rendering code (form layout, field cycling, buttons)
- Single mode instead of 6+ modes (further consolidates after Phase 2's ViewEdit unification)
- Makes save explicit ('S' key, like ViewEdit proposes)
- Enables preview of parse results and automatic assignments

### 4. Feature Request Context

Top open requests (from `aglet-features.ag`):

| ID | Area | Title | Status |
|----|------|-------|--------|
| #1 | UX | Add-panel like Edit-panel | Open |
| #2 | Validation | Section compatibility validation | Open |
| #3 | UX | Use S to save (not Enter) | Open |
| #4 | UX | Save-on-exit prompt | Open |
| #5 | CLI | CLI feature parity with TUI | Open |
| #6 | UX | Views without criteria | **Done** |

The InputPanel abstraction directly addresses #1 and aligns with #3 (#4 follows from explicit save).

### 5. Design Principles Preserved

The proposal follows the **TUI UX Redesign (tui-ux-redesign.md)** principles:

- **P1**: Esc always means "go back." InputPanel: Esc cancels, no side effects.
- **P3**: Same key = same thing. InputPanel: all input forms use Tab to cycle, Enter to activate buttons, S to save.
- **P4**: Shared abstraction. InputPanel: one form class for add/edit/name, reusing TextBuffer.

### 6. Implementation Sequence (Phases 5a–5e)

```
Phase 5a: Create InputPanel abstraction
Phase 5b: Migrate AddInput → InputPanel(AddItem)
Phase 5c: Migrate ItemEdit → InputPanel(EditItem)
Phase 5d: Migrate name inputs → InputPanel(NameInput)
Phase 5e: Change save key to 'S' (capital)
```

Each phase is testable independently; keeps existing tests passing throughout.

---

## Recommendations

1. **Review the full spec**: `docs/specs/proposals/tui-add-workflow-analysis.md` has examples, risks, and open questions.

2. **Pilot with add workflow**: Start with Phase 5a–5b. This immediately addresses feature request #1 and provides user feedback without breaking existing edit workflow.

3. **Measure discoverability**: After Phase 5b, survey users: "Is adding an item with categories easier/more discoverable now?"

4. **Extend to CLI**: The CLI currently lacks view/section richness (feature request #5). An InputPanel-inspired CLI subcommand could help:
   ```
   agenda-cli item create "Fix login timeout" --note "..." --categories "High,Infrastructure"
   ```

5. **Defer Phase 5e** (S key) until 5a–5d are stable. Breaking Enter behavior requires migration guide.

---

## Related Specs

- **tui-ux-redesign.md**: Broader redesign (30→21 modes). InputPanel follows same consolidation philosophy.
- **tui-view-workflow-implementation.md**: Current contract. InputPanel doesn't change it; just improves add/edit UX.
- **feature-requests.md**: Original feature list (markdown). Updated as `aglet-features.ag` (database).

---

## Next Steps

1. Socialize InputPanel design with team
2. Prioritize Phase 5 timeline
3. Implement Phase 5a (InputPanel abstraction) as spike/POC
4. Gather user feedback on Phase 5b (add workflow)
5. Plan Phase 5e (save key) as separate milestone
