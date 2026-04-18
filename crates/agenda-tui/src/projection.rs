use crate::*;

use agenda_core::query::matches_text_search;
use agenda_core::store::DEFAULT_VIEW_NAME;
use agenda_core::workflow::{
    build_ready_queue_view, claimable_item_ids, resolve_workflow_config, READY_QUEUE_VIEW_NAME,
};

pub(crate) fn project_slots(app: &mut App, store: &Store, items: &[Item]) -> TuiResult<Vec<Slot>> {
    let mut slots = Vec::new();
    if app.views.is_empty() {
        slots.push(Slot {
            title: "All Items (no views configured)".to_string(),
            items: items.to_vec(),
            context: SlotContext::Unmatched,
        });
        if app.mode == Mode::Normal {
            app.status = "No views configured; showing fallback item list".to_string();
        }
        app.set_active_view_index(0);
    } else {
        app.set_active_view_index(app.view_index);
        let view = app
            .current_view()
            .cloned()
            .ok_or("No active view".to_string())?;
        let reference_date = jiff::Zoned::now().date();
        let view_items = if view.name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
            if let Some(workflow) = resolve_workflow_config(store)? {
                let claimable_ids = claimable_item_ids(store, items, workflow)?;
                items
                    .iter()
                    .filter(|item| claimable_ids.contains(&item.id))
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            items.to_vec()
        };
        let mut result = resolve_view(&view, &view_items, &app.categories, reference_date);
        if app.effective_hide_dependent_items() {
            for section in &mut result.sections {
                section.items.retain(|item| !app.is_item_blocked(item.id));
                for subsection in &mut section.subsections {
                    subsection
                        .items
                        .retain(|item| !app.is_item_blocked(item.id));
                }
            }
            if let Some(unmatched_items) = &mut result.unmatched {
                unmatched_items.retain(|item| !app.is_item_blocked(item.id));
            }
        }

        for section in result.sections {
            if section.subsections.is_empty() {
                slots.push(Slot {
                    title: section.title,
                    items: section.items,
                    context: SlotContext::Section {
                        section_index: section.section_index,
                    },
                });
                continue;
            }

            for subsection in section.subsections {
                slots.push(Slot {
                    title: format!("{} / {}", section.title, subsection.title),
                    items: subsection.items,
                    context: SlotContext::GeneratedSection {
                        section_index: section.section_index,
                        on_insert_assign: subsection.on_insert_assign,
                        on_remove_unassign: subsection.on_remove_unassign,
                    },
                });
            }
        }

        if let Some(unmatched_items) = result.unmatched {
            if should_render_unmatched_lane(&unmatched_items) {
                slots.push(Slot {
                    title: result
                        .unmatched_label
                        .unwrap_or_else(|| "Unassigned".to_string()),
                    items: unmatched_items,
                    context: SlotContext::Unmatched,
                });
            }
        }

        if slots.is_empty() {
            slots.push(Slot {
                title: "No visible sections".to_string(),
                items: Vec::new(),
                context: SlotContext::Unmatched,
            });
        }
    }

    if app.section_filters.len() != slots.len() {
        app.section_filters = vec![None; slots.len()];
        app.search_buffer.clear();
    }
    if app.slot_sort_keys.len() != slots.len() {
        app.slot_sort_keys = vec![Vec::new(); slots.len()];
    }

    let active_view = app.current_view().cloned();
    let category_names_lower_ascii: HashMap<CategoryId, String> = app
        .categories
        .iter()
        .map(|category| (category.id, category.name.to_ascii_lowercase()))
        .collect();

    for (slot_index, (slot, filter)) in slots.iter_mut().zip(app.section_filters.iter()).enumerate()
    {
        if let Some(needle) = filter {
            let needle = needle.to_ascii_lowercase();
            slot.items.retain(|item| {
                matches_text_search(item, &needle, Some(&category_names_lower_ascii))
            });
        }

        let mut sort_keys = app.slot_sort_keys[slot_index].clone();
        sort_keys
            .retain(|key| app.slot_sort_key_is_valid_for_slot(active_view.as_ref(), slot, key));
        if sort_keys != app.slot_sort_keys[slot_index] {
            app.slot_sort_keys[slot_index] = sort_keys.clone();
        }
        if !sort_keys.is_empty() {
            app.sort_slot_items(slot, &sort_keys);
        }
    }

    Ok(slots)
}

pub(crate) fn load_views_with_ready_queue(store: &Store) -> TuiResult<Vec<View>> {
    let mut views = store.list_views()?;
    if let Some(workflow) = resolve_workflow_config(store)? {
        let ready_queue_view = build_ready_queue_view(store, workflow)?;
        let insert_at = views
            .iter()
            .position(|view| view.name.eq_ignore_ascii_case(DEFAULT_VIEW_NAME))
            .map(|index| index + 1)
            .unwrap_or(0)
            .min(views.len());
        views.insert(insert_at, ready_queue_view);
    }
    Ok(views)
}
