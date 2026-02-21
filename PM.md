# PM & Grooming Process

Break down tasks into items suitable for execution by parallel coding agents.

## Aglet CLI Reference

```bash
agenda-cli list                          # All open items
agenda-cli list --view "High Priority"   # Items matching a view
agenda-cli list --category CLI           # Items in a category
agenda-cli list --include-done           # Include completed items
agenda-cli show <id>                     # Item detail with categories
agenda-cli search <keyword>             # Search text and notes

agenda-cli add "Title" --note "Details"  # Create item
agenda-cli edit <id> "New title"         # Edit item text
agenda-cli edit <id> --note "..."        # Edit item note
agenda-cli edit <id> --done true         # Mark complete
agenda-cli delete <id>                   # Delete item

agenda-cli category list                 # Show category tree
agenda-cli category assign <id> <name>   # Assign category to item
agenda-cli category unassign <id> <name> # Remove category from item

agenda-cli view list                     # List saved views
agenda-cli view show "View Name"         # Show view contents
agenda-cli view create "Name" --include Cat1 --include Cat2
```

Categories are used for priority, status, area, etc. Views are saved filters. Use `--include` filters (AND-based) to create views.

## Grooming Process

### 1. Assess Current State

```bash
agenda-cli list                          # Review all items
agenda-cli view show "Pending"           # What needs work
agenda-cli view show "High Priority"     # Critical items
agenda-cli search "keyword"              # Find related items
```

### 2. Review Each Item

**Checklist:**
- Clear, specific title (imperative mood: "Add X", "Fix Y")
- Note explaining WHAT, WHY, and acceptance criteria
- Correct categories assigned (Priority, Status, Area)
- Small enough to implement in ~10-15 minutes, or broken into subtasks

**Priority (exclusive):** High, Medium, Low
**Status (exclusive):** Pending, In Progress, Completed, Deferred
**Area (non-exclusive):** CLI, UX, Validation, Display, Automation

### 3. Common Actions

**Add description to an item:**
```bash
agenda-cli edit <id> --note "What: Clear description of work.
Why: Reason this matters.
Acceptance Criteria:
- Criterion 1
- Criterion 2"
```

**Break down a large task:**
Create smaller items and mark the parent done or update its note to reference them.

```bash
agenda-cli add "Feature: Step 1 - Foundation" --note "..."
agenda-cli category assign <id> High
agenda-cli category assign <id> Pending

agenda-cli add "Feature: Step 2 - Core logic" --note "..."
agenda-cli add "Feature: Step 3 - Integration" --note "..."
```

**Mark complete:**
```bash
agenda-cli edit <id> --done true
```

### 4. Task Sizing

- **Good:** 5-15 minutes, single focus, clear when done
- **Too large:** >5 acceptance criteria, touches >5 files, has "and also" in description → break it down
- **Too small:** Consider combining with related work

### 5. Subtask Naming

Use a common prefix to group related subtasks:

```
/edit: Parse command and extract buffer
/edit: Open buffer in external editor
/edit: Validate edited buffer
/edit: Merge buffer back into conversation
```

### 6. Questions & Decisions

For items needing architectural decisions, add a note documenting the question and options. Consider creating a separate "Design: ..." item assigned High priority to resolve it before implementation items.

Track open questions in `docs/questions.md`:

```markdown
## Feature Name
**Related Items:** <ids>
**Question:** How should we handle X?
Options:
1. Option A - pros/cons
2. Option B - pros/cons
```

### 7. After Grooming

```bash
agenda-cli view show "Pending"    # Should show well-defined items
agenda-cli list                   # Scan titles for clarity
```

Commit and push your changes.

## Quality Indicators

**Healthy backlog:** Most items have notes, tasks are small, pending items are actionable.

**Needs grooming:** Items without descriptions, large undivided tasks, unclear priorities.
