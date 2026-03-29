# Set Category Search-or-Create Mockup

This mockup updates the board column `Set Category` popup to behave like a real
search-or-create field:

- The input label becomes `Search or create>`
- Spaces are accepted as normal text while the query field is focused
- `Enter` does not open a confirmation prompt
- `Enter` reuses an exact existing child category, otherwise creates a new one
  under the current column parent and saves immediately
- `Tab` moves to the category list for manual browse/toggle behavior

## Recommended UI

```text
┌Set Category────────────────────────────────────────────────────────────────────┐
│ Column: Project  Mode: multi                                                  │
│ Selected: Death Star                                                          │
│ ┌Item Context────────────────────────────────────────────────────────────────┐ │
│ │ Verify Death Star design                                                  │ │
│ └────────────────────────────────────────────────────────────────────────────┘ │
│ ┌Search or Create────────────────────────────────────────────────────────────┐ │
│ │ Search or create> Death Star II█                                          │ │
│ │ Enter reuses an exact match or creates a new child category and saves it. │ │
│ └────────────────────────────────────────────────────────────────────────────┘ │
│ ┌Categories──────────────────────────────────────────────────────────────────┐ │
│ │ > [x] Death Star                                                          │ │
│ │   [ ] Death Star II                                                       │ │
│ │   [ ] Misc Project                                                        │ │
│ │   [ ] Sample Project                                                      │ │
│ └────────────────────────────────────────────────────────────────────────────┘ │
│ Type to search or name a new category | Tab list | Space toggle in list      │
│ Enter use/create+save | Esc cancel                                           │
└────────────────────────────────────────────────────────────────────────────────┘
```

## No Exact Match

```text
┌Set Category────────────────────────────────────────────────────────────────────┐
│ Column: Project  Mode: multi                                                  │
│ Selected: Death Star                                                          │
│ ┌Item Context────────────────────────────────────────────────────────────────┐ │
│ │ Verify Death Star design                                                  │ │
│ └────────────────────────────────────────────────────────────────────────────┘ │
│ ┌Search or Create────────────────────────────────────────────────────────────┐ │
│ │ Search or create> New Project█                                            │ │
│ │ Enter reuses an exact match or creates a new child category and saves it. │ │
│ └────────────────────────────────────────────────────────────────────────────┘ │
│ ┌Categories──────────────────────────────────────────────────────────────────┐ │
│ │ (no exact match yet; Enter will create this as a new child)               │ │
│ └────────────────────────────────────────────────────────────────────────────┘ │
│ Type to search or name a new category | Tab list | Enter use/create+save     │
│ Esc cancel                                                                    │
└────────────────────────────────────────────────────────────────────────────────┘
```

Result:

```text
Created category 'New Project' and saved column edits for 'Verify Death Star design'
```
