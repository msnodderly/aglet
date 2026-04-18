use std::path::{Path, PathBuf};

use uuid::Uuid;

/// App-settings key for the notes directory override (relative path).
pub const NOTES_DIR_SETTING_KEY: &str = "notes_dir";

/// App-settings key controlling whether new items are file-backed by default.
pub const NEW_ITEMS_LINKED_BY_DEFAULT_KEY: &str = "new_items_linked_by_default";

/// Deprecated: use `NEW_ITEMS_LINKED_BY_DEFAULT_KEY`. Retained for one-time
/// migration of old settings DBs.
pub const NOTES_DEFAULT_LINKED_KEY: &str = "notes_default_linked";

/// Reads `NEW_ITEMS_LINKED_BY_DEFAULT_KEY`, migrating from the deprecated
/// `NOTES_DEFAULT_LINKED_KEY` on first access. Returns the value string,
/// or `None` if neither key is set.
pub fn read_new_items_linked_by_default(store: &crate::store::Store) -> Option<String> {
    match store
        .get_app_setting(NEW_ITEMS_LINKED_BY_DEFAULT_KEY)
        .ok()
        .flatten()
    {
        Some(v) => Some(v),
        None => {
            let old = store
                .get_app_setting(NOTES_DEFAULT_LINKED_KEY)
                .ok()
                .flatten();
            if let Some(ref old_val) = old {
                let _ = store.set_app_setting(NEW_ITEMS_LINKED_BY_DEFAULT_KEY, old_val);
                let _ = store.delete_app_setting(NOTES_DEFAULT_LINKED_KEY);
            }
            old
        }
    }
}

/// Convert text to a filename-safe slug.
///
/// Lowercase, non-alphanumeric characters replaced with hyphens,
/// consecutive hyphens collapsed, leading/trailing hyphens trimmed,
/// truncated to 60 characters.
pub fn slugify(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    let mut prev_hyphen = true; // suppress leading hyphen
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            for lower in ch.to_lowercase() {
                slug.push(lower);
            }
            prev_hyphen = false;
        } else if !prev_hyphen {
            slug.push('-');
            prev_hyphen = true;
        }
    }
    // Trim trailing hyphen
    let trimmed = slug.trim_end_matches('-');
    if trimmed.len() > 60 {
        // Truncate at a hyphen boundary if possible
        let cut = &trimmed[..60];
        match cut.rfind('-') {
            Some(pos) if pos > 20 => cut[..pos].to_string(),
            _ => cut.to_string(),
        }
    } else {
        trimmed.to_string()
    }
}

/// Generate the note filename for an item: `{slug}-{first 8 hex chars of UUID}.md`.
pub fn note_filename(text: &str, item_id: Uuid) -> String {
    let slug = slugify(text);
    let id_prefix = &item_id.as_simple().to_string()[..8];
    if slug.is_empty() {
        format!("{id_prefix}.md")
    } else {
        format!("{slug}-{id_prefix}.md")
    }
}

/// Resolve the notes directory path.
///
/// If `override_dir` is provided, it is resolved relative to the parent of `db_path`.
/// Otherwise, the default convention is `<db_stem>-notes/` as a sibling of the `.ag` file.
pub fn resolve_notes_dir(db_path: &Path, override_dir: Option<&str>) -> PathBuf {
    let parent = db_path.parent().unwrap_or(Path::new("."));
    match override_dir {
        Some(dir) => parent.join(dir),
        None => {
            let stem = db_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("agenda");
            parent.join(format!("{stem}-notes"))
        }
    }
}

/// Resolve the full path to a specific note file.
pub fn resolve_note_path(db_path: &Path, override_dir: Option<&str>, filename: &str) -> PathBuf {
    resolve_notes_dir(db_path, override_dir).join(filename)
}

/// Default template content for a newly created linked note file.
pub fn note_template(item_text: &str) -> String {
    format!("# {item_text}\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Build auth middleware"), "build-auth-middleware");
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!(slugify("Hello, World! (v2)"), "hello-world-v2");
    }

    #[test]
    fn slugify_consecutive_separators() {
        assert_eq!(slugify("foo---bar   baz"), "foo-bar-baz");
    }

    #[test]
    fn slugify_empty() {
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn slugify_unicode() {
        assert_eq!(slugify("café résumé"), "caf-r-sum");
    }

    #[test]
    fn slugify_long_string() {
        let long = "a".repeat(100);
        let result = slugify(&long);
        assert!(result.len() <= 60);
    }

    #[test]
    fn note_filename_basic() {
        let id = Uuid::parse_str("a3f8b2c1-1234-5678-9abc-def012345678").unwrap();
        let name = note_filename("Build auth middleware", id);
        assert_eq!(name, "build-auth-middleware-a3f8b2c1.md");
    }

    #[test]
    fn note_filename_empty_text() {
        let id = Uuid::parse_str("a3f8b2c1-1234-5678-9abc-def012345678").unwrap();
        let name = note_filename("", id);
        assert_eq!(name, "a3f8b2c1.md");
    }

    #[test]
    fn resolve_notes_dir_default() {
        let db = Path::new("/home/user/myproject.ag");
        let dir = resolve_notes_dir(db, None);
        assert_eq!(dir, PathBuf::from("/home/user/myproject-notes"));
    }

    #[test]
    fn resolve_notes_dir_override() {
        let db = Path::new("/home/user/myproject.ag");
        let dir = resolve_notes_dir(db, Some("../shared-notes"));
        assert_eq!(dir, PathBuf::from("/home/user/../shared-notes"));
    }

    #[test]
    fn resolve_note_path_combines_dir_and_file() {
        let db = Path::new("/home/user/myproject.ag");
        let path = resolve_note_path(db, None, "build-auth-a3f8b2c1.md");
        assert_eq!(
            path,
            PathBuf::from("/home/user/myproject-notes/build-auth-a3f8b2c1.md")
        );
    }

    #[test]
    fn note_template_format() {
        let content = note_template("Build auth middleware");
        assert_eq!(content, "# Build auth middleware\n\n");
    }

    #[test]
    fn slugify_only_special_chars() {
        // All non-alphanumeric input yields empty slug
        assert_eq!(slugify("!@#$%^&*()"), "");
    }

    #[test]
    fn slugify_leading_trailing_whitespace() {
        assert_eq!(slugify("  hello world  "), "hello-world");
    }

    #[test]
    fn slugify_single_char() {
        assert_eq!(slugify("A"), "a");
    }

    #[test]
    fn slugify_long_truncates_at_hyphen_boundary() {
        // 70+ chars with hyphens — should truncate at a hyphen boundary within 60 chars
        let input = "this-is-a-very-long-item-name-that-exceeds-the-sixty-character-limit-by-a-lot";
        let result = slugify(input);
        assert!(result.len() <= 60);
        // Should not end with a hyphen
        assert!(!result.ends_with('-'));
    }

    #[test]
    fn slugify_numbers_preserved() {
        assert_eq!(slugify("Sprint 42 review"), "sprint-42-review");
    }

    #[test]
    fn note_filename_special_chars_only_text() {
        // When slug is empty, filename is just the UUID prefix
        let id = Uuid::parse_str("a3f8b2c1-1234-5678-9abc-def012345678").unwrap();
        let name = note_filename("!!!", id);
        assert_eq!(name, "a3f8b2c1.md");
    }

    #[test]
    fn resolve_notes_dir_no_extension() {
        // DB path without .ag extension
        let db = Path::new("/home/user/myproject");
        let dir = resolve_notes_dir(db, None);
        assert_eq!(dir, PathBuf::from("/home/user/myproject-notes"));
    }

    #[test]
    fn resolve_notes_dir_bare_filename() {
        // DB path is just a filename (no directory component)
        let db = Path::new("tasks.ag");
        let dir = resolve_notes_dir(db, None);
        assert_eq!(dir, PathBuf::from("tasks-notes"));
    }
}
