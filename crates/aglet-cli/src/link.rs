//! `link` and `unlink` commands.

use crate::*;

pub(crate) fn cmd_link(aglet: &Aglet<'_>, command: LinkCommand) -> CliResult {
    match command {
        LinkCommand::DependsOn {
            item_id,
            depends_on_item_id,
        } => {
            let item_id = resolve_item_id(&item_id, aglet.store())?;
            let depends_on_item_id = resolve_item_id(&depends_on_item_id, aglet.store())?;
            let result = aglet.link_items_depends_on(item_id, depends_on_item_id)?;
            if result.created {
                println!("linked {} depends-on {}", item_id, depends_on_item_id);
            } else {
                println!(
                    "link already exists: {} depends-on {}",
                    item_id, depends_on_item_id
                );
            }
            Ok(())
        }
        LinkCommand::Blocks {
            blocker_item_id,
            blocked_item_id,
        } => {
            let blocker_item_id = resolve_item_id(&blocker_item_id, aglet.store())?;
            let blocked_item_id = resolve_item_id(&blocked_item_id, aglet.store())?;
            let result = aglet.link_items_blocks(blocker_item_id, blocked_item_id)?;
            if result.created {
                println!("linked {} blocks {}", blocker_item_id, blocked_item_id);
            } else {
                println!(
                    "link already exists: {} blocks {}",
                    blocker_item_id, blocked_item_id
                );
            }
            Ok(())
        }
        LinkCommand::Related {
            item_a_id,
            item_b_id,
        } => {
            let item_a_id = resolve_item_id(&item_a_id, aglet.store())?;
            let item_b_id = resolve_item_id(&item_b_id, aglet.store())?;
            let result = aglet.link_items_related(item_a_id, item_b_id)?;
            if result.created {
                println!("linked {} related {}", item_a_id, item_b_id);
            } else {
                println!("link already exists: {} related {}", item_a_id, item_b_id);
            }
            Ok(())
        }
    }
}

pub(crate) fn cmd_unlink(aglet: &Aglet<'_>, command: UnlinkCommand) -> CliResult {
    match command {
        UnlinkCommand::Blocks {
            blocker_item_id,
            blocked_item_id,
        } => unlink_blocks(aglet, blocker_item_id, blocked_item_id),
        UnlinkCommand::DependsOn {
            item_id,
            depends_on_item_id,
        } => unlink_depends_on(aglet, item_id, depends_on_item_id),
        UnlinkCommand::Related {
            item_a_id,
            item_b_id,
        } => unlink_related(aglet, item_a_id, item_b_id),
    }
}

pub(crate) fn unlink_depends_on(
    aglet: &Aglet<'_>,
    item_id: String,
    depends_on_item_id: String,
) -> CliResult {
    let item_id = resolve_item_id(&item_id, aglet.store())?;
    let depends_on_item_id = resolve_item_id(&depends_on_item_id, aglet.store())?;
    aglet.unlink_items_depends_on(item_id, depends_on_item_id)?;
    println!("unlinked {} depends-on {}", item_id, depends_on_item_id);
    Ok(())
}

pub(crate) fn unlink_blocks(
    aglet: &Aglet<'_>,
    blocker_item_id: String,
    blocked_item_id: String,
) -> CliResult {
    let blocker_item_id = resolve_item_id(&blocker_item_id, aglet.store())?;
    let blocked_item_id = resolve_item_id(&blocked_item_id, aglet.store())?;
    aglet.unlink_items_blocks(blocker_item_id, blocked_item_id)?;
    println!("unlinked {} blocks {}", blocker_item_id, blocked_item_id);
    Ok(())
}

pub(crate) fn unlink_related(aglet: &Aglet<'_>, item_a_id: String, item_b_id: String) -> CliResult {
    let item_a_id = resolve_item_id(&item_a_id, aglet.store())?;
    let item_b_id = resolve_item_id(&item_b_id, aglet.store())?;
    aglet.unlink_items_related(item_a_id, item_b_id)?;
    println!("unlinked {} related {}", item_a_id, item_b_id);
    Ok(())
}

pub(crate) fn item_link_section_lines(store: &Store, item_id: ItemId) -> CliResult<Vec<String>> {
    let prereqs = resolve_link_neighbors(store, store.list_dependency_ids_for_item(item_id)?)?;
    let dependents = resolve_link_neighbors(store, store.list_dependent_ids_for_item(item_id)?)?;
    let related = resolve_link_neighbors(store, store.list_related_ids_for_item(item_id)?)?;

    let mut lines = Vec::new();
    append_link_section_lines(&mut lines, "prereqs", &prereqs);
    append_link_section_lines(&mut lines, "dependents (blocks)", &dependents);
    append_link_section_lines(&mut lines, "related", &related);
    Ok(lines)
}

pub(crate) fn resolve_link_neighbors(
    store: &Store,
    ids: Vec<ItemId>,
) -> CliResult<Vec<(String, String)>> {
    let mut rows = Vec::new();
    for id in ids {
        match store.get_item(id) {
            Ok(item) => rows.push((
                item.text.to_ascii_lowercase(),
                format!(
                    "  {} | {} | {}",
                    id,
                    if item.is_done { "done" } else { "open" },
                    item.text
                ),
            )),
            Err(_) => rows.push((
                id.to_string(),
                format!("  {} | missing | (linked item unavailable)", id),
            )),
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(rows)
}

pub(crate) fn append_link_section_lines(
    lines: &mut Vec<String>,
    label: &str,
    rows: &[(String, String)],
) {
    if rows.is_empty() {
        lines.push(format!("{label}: (none)"));
        return;
    }
    lines.push(format!("{label}:"));
    for (_, line) in rows {
        lines.push(line.clone());
    }
}
