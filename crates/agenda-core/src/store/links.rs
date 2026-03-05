use rusqlite::{params, OptionalExtension};

use crate::error::Result;
use crate::model::{ItemId, ItemLink, ItemLinkKind};

use super::Store;

impl Store {
    // ── Item link persistence ──────────────────────────────────

    pub fn create_item_link(&self, link: &ItemLink) -> Result<()> {
        self.conn.execute(
            "INSERT INTO item_links (item_id, other_item_id, kind, created_at, origin)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                link.item_id.to_string(),
                link.other_item_id.to_string(),
                Self::item_link_kind_to_db(link.kind),
                link.created_at.to_rfc3339(),
                link.origin,
            ],
        )?;
        Ok(())
    }

    pub fn delete_item_link(
        &self,
        item_id: ItemId,
        other_item_id: ItemId,
        kind: ItemLinkKind,
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM item_links
             WHERE item_id = ?1 AND other_item_id = ?2 AND kind = ?3",
            params![
                item_id.to_string(),
                other_item_id.to_string(),
                Self::item_link_kind_to_db(kind),
            ],
        )?;
        Ok(())
    }

    pub fn item_link_exists(
        &self,
        item_id: ItemId,
        other_item_id: ItemId,
        kind: ItemLinkKind,
    ) -> Result<bool> {
        let exists: Option<i32> = self
            .conn
            .query_row(
                "SELECT 1 FROM item_links
                 WHERE item_id = ?1 AND other_item_id = ?2 AND kind = ?3
                 LIMIT 1",
                params![
                    item_id.to_string(),
                    other_item_id.to_string(),
                    Self::item_link_kind_to_db(kind),
                ],
                |row| row.get(0),
            )
            .optional()?;
        Ok(exists.is_some())
    }

    /// Immediate prerequisites for a dependent item (outbound depends-on edges).
    pub fn list_dependency_ids_for_item(&self, item_id: ItemId) -> Result<Vec<ItemId>> {
        let mut stmt = self.conn.prepare(
            "SELECT other_item_id
             FROM item_links
             WHERE item_id = ?1 AND kind = 'depends-on'
             ORDER BY created_at ASC, other_item_id ASC",
        )?;
        let rows = stmt
            .query_map(params![item_id.to_string()], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        rows.into_iter()
            .map(|raw| Self::parse_uuid_from_db_text(&raw, "item_links.other_item_id"))
            .collect()
    }

    /// Immediate dependents of an item (inbound depends-on edges; inverse "blocks" view).
    pub fn list_dependent_ids_for_item(&self, item_id: ItemId) -> Result<Vec<ItemId>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_id
             FROM item_links
             WHERE other_item_id = ?1 AND kind = 'depends-on'
             ORDER BY created_at ASC, item_id ASC",
        )?;
        let rows = stmt
            .query_map(params![item_id.to_string()], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        rows.into_iter()
            .map(|raw| Self::parse_uuid_from_db_text(&raw, "item_links.item_id"))
            .collect()
    }

    /// Immediate related items (symmetric query over normalized `related` rows).
    pub fn list_related_ids_for_item(&self, item_id: ItemId) -> Result<Vec<ItemId>> {
        let mut stmt = self.conn.prepare(
            "SELECT CASE WHEN item_id = ?1 THEN other_item_id ELSE item_id END AS neighbor_id
             FROM item_links
             WHERE kind = 'related' AND (item_id = ?1 OR other_item_id = ?1)
             ORDER BY created_at ASC, neighbor_id ASC",
        )?;
        let rows = stmt
            .query_map(params![item_id.to_string()], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        rows.into_iter()
            .map(|raw| Self::parse_uuid_from_db_text(&raw, "item_links.related_neighbor_id"))
            .collect()
    }

    /// Optional convenience for `agenda show` / TUI panels.
    pub fn list_item_links_for_item(&self, item_id: ItemId) -> Result<Vec<ItemLink>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_id, other_item_id, kind, created_at, origin
             FROM item_links
             WHERE item_id = ?1 OR other_item_id = ?1
             ORDER BY created_at ASC, item_id ASC, other_item_id ASC, kind ASC",
        )?;
        let rows = stmt
            .query_map(params![item_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        rows.into_iter()
            .map(
                |(item_id_str, other_item_id_str, kind_str, created_at_str, origin)| {
                    Self::item_link_from_db_row(
                        &item_id_str,
                        &other_item_id_str,
                        &kind_str,
                        &created_at_str,
                        origin,
                    )
                },
            )
            .collect()
    }
}
