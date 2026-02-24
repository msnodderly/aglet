# Lotus Agenda: Creating a New View & Adding a Section — Step by Step

## Concrete Example

Suppose you have categories `Priority` (with children `High`, `Medium`, `Low`), `People` (with children `Fred Smith`, `Sally Jones`), and `Phone Calls`. You want to create a view called **"My Calls"** that shows phone call items organized by priority, then add a section for a specific person.

---

## Part 1: Create a New View via the View Manager

### Method A — Through the View Manager (F8)

1. **`F8`** — Opens the View Manager, which lists all views in the file.
2. **`INS`** — Opens the **View Add box** (a dialog with settings fields).
3. In the **View name** field, type: `My Calls` then **`↓`** to move to the next field.
4. In the **Type** field, leave it as `Standard` (press **`Space`** to toggle if needed).
5. **`↓`** to the **Section(s)** field.
6. **`F3`** (CHOICES) — Pops up the category hierarchy. Use **`↓`/`↑`** to highlight `High`, press **`Enter`** to select it. Type `,` then press **`F3`** again, select `Medium`, **`Enter`**, then `Low`.
   - Alternatively, just type `High, Medium, Low` directly — Agenda's category matching will resolve each name.
7. Optionally set **Hide empty sections** to `Yes`, **Section sorting** to `Category order`, etc.
8. Set the **Filter** field: **`F3`**, select `Phone Calls`, **`Enter`**. This restricts the view to only items assigned to both a priority category and "Phone Calls."
9. **`Enter`** — Confirms and creates the view.

**Result:** Agenda displays "My Calls" with three sections headed `High`, `Medium`, and `Low`, showing only items that are assigned to both a priority category and "Phone Calls."

### Method B — Through the Menu

1. **`F10`** — Opens the main menu bar.
2. **`V`** (or arrow to `View`) → **`Enter`**
3. **`A`** (or arrow to `Add`) → **`Enter`**
4. Same dialog box appears — fill in as above.

### View Add Box Settings Reference

| Setting | Description |
|---|---|
| **View name** | 1–37 character unique name for the view |
| **Type** | `Standard` (general purpose) or `Datebook` (date/time organized) |
| **Section(s)** | List of categories to appear as section heads |
| **Item sorting** | Default sort order for items in all sections |
| **Section sorting** | `None`, `Category order`, `Alphabetic`, or `Numeric` |
| **Hide empty sections** | `Yes`/`No` — whether to show sections with no items |
| **Hide done items** | `Yes`/`No` — whether to hide items marked done |
| **Filter** | Optional criteria to restrict which items appear in the view |

---

## Part 2: Add a New Section to the Existing View

Now you want to add a "Fred Smith" section below the "Low" section.

### Method A — Full Dialog (F10 Menu)

1. Place the highlight (bar cursor) anywhere in the **Low** section (the section adjacent to where you want the new one).
2. **`F10`** → **`V`** (View) → **`S`** (Section) → **`A`** (Add) — Opens the **Section Add box**.
3. In the **Section head** field, type `Fred` — Agenda's category matching shows `Fred Smith` at the top of the screen as a match. Press **`Enter`** to accept it.
   - Or press **`F3`** (CHOICES), navigate to `Fred Smith` in the hierarchy, and **`Enter`**.
4. Set **Insert** to `Below current section` (the default).
5. Optionally configure the **Filter**, **Item sorting**, or **Columns** fields.
6. **`Enter`** — Confirms and adds the section.

### Method B — Shortcut (ALT-D / ALT-U)

1. Highlight anywhere in the section *above* where you want the new section.
2. **`Alt+D`** — Prompts directly for a section head (skips the full dialog, uses defaults).
3. Type `Fred` → category matching shows `Fred Smith` → **`Enter`**.
4. Section appears immediately below.

**To add *above* instead:** Use **`Alt+U`** instead of `Alt+D`.

### Section Add Box Settings Reference

| Setting | Description |
|---|---|
| **Section head** | Category used as the section head (new or existing) |
| **Insert** | `Below current section` or `Above current section` |
| **Item sorting** | Sort order for items in this section (overrides view default) |
| **Filter** | Optional criteria to further restrict items in this section |
| **Columns** | List of categories to display as column heads in this section |

---

## Key Interaction: Category Matching

When you type a category name (e.g., `Fre...`), Agenda shows at the top of the screen:
- How many categories match your typed string so far
- The first matching category name

You can then:
- **`Enter`** — Accept the displayed match
- **`←`/`→`** — Cycle through other matches
- **`F3`** — Pop up the hierarchy to browse visually
- **Keep typing** — Narrow matches further, or type a unique string to create a *new* category on the fly

This is the same mechanism used everywhere categories are selected — section heads, column heads, filters, and assignments.

---

## View Manager Quick Reference

Once in the View Manager (**`F8`**):

| Action | Key |
|---|---|
| Switch to highlighted view | `Enter` |
| Leave the view manager | `ESC` |
| Add a view | `INS` |
| Edit a view name | `F2` (EDIT) |
| Delete highlighted view | `F4` or `Alt+F4` |
| Display View Properties box | `F6` (PROPS) |
| Sort view names alphabetically | `Alt+F5` (SORT) |
| Copy highlighted view | `Alt+F9` (COPY) |
| Reposition a view in the list | `Alt+F10` (MOVE) |

## Section Commands Quick Reference

| Action | Key |
|---|---|
| Add section below | `Alt+D` (shortcut) or `F10` → View → Section → Add |
| Add section above | `Alt+U` |
| Remove section | `DEL` or `F10` → View → Section → Remove |
| Move/reorder section | `Alt+F10` (MOVE) |
| Section properties | `F6` on section head, or `F10` → View → Section → Properties |
