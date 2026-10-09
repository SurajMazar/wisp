//! Skills and their versions (spec §64). Content is stored verbatim; parsing lives in the core.

use rusqlite::{OptionalExtension, Row, params};
use sha2::{Digest, Sha256};
use wisp_ipc::types::skills::{SkillSource, SkillVersion};

use crate::{Database, DbResult};

#[derive(Debug, Clone, PartialEq)]
pub struct StoredSkill {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub source: SkillSource,
    pub enabled: bool,
    pub version: u32,
    pub content: String,
    pub updated_at_ms: i64,
}

fn source_str(s: SkillSource) -> &'static str {
    match s {
        SkillSource::Builtin => "builtin",
        SkillSource::User => "user",
        SkillSource::Imported => "imported",
    }
}

fn from_row(r: &Row<'_>) -> rusqlite::Result<StoredSkill> {
    let source: String = r.get("source")?;
    Ok(StoredSkill {
        id: r.get("id")?,
        slug: r.get("slug")?,
        name: r.get("name")?,
        description: r.get("description")?,
        source: match source.as_str() {
            "builtin" => SkillSource::Builtin,
            "imported" => SkillSource::Imported,
            _ => SkillSource::User,
        },
        enabled: r.get::<_, i64>("enabled")? == 1,
        version: r.get("version")?,
        content: r.get("content")?,
        updated_at_ms: r.get("updated_at")?,
    })
}

const SELECT: &str = "SELECT s.id, s.slug, s.name, s.description, s.source, s.enabled, s.updated_at, v.version, v.content
     FROM skills s JOIN skill_versions v ON v.id = s.current_version_id";

fn sha(content: &str) -> String {
    hex::encode(Sha256::digest(content.as_bytes()))
}

impl Database {
    pub fn list_skills(&self) -> DbResult<Vec<StoredSkill>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY s.name COLLATE NOCASE"))?;
        Ok(stmt
            .query_map([], from_row)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get_skill(&self, id: &str) -> DbResult<Option<StoredSkill>> {
        Ok(self
            .lock()?
            .query_row(&format!("{SELECT} WHERE s.id = ?1"), [id], from_row)
            .optional()?)
    }

    pub fn skill_slug_exists(&self, slug: &str) -> DbResult<bool> {
        Ok(self
            .lock()?
            .query_row("SELECT 1 FROM skills WHERE slug = ?1", [slug], |_| Ok(()))
            .optional()?
            .is_some())
    }

    /// Create a skill with version 1. The slug gets a numeric suffix if it's taken.
    pub fn create_skill(
        &self,
        slug: &str,
        name: &str,
        description: &str,
        source: SkillSource,
        enabled: bool,
        content: &str,
    ) -> DbResult<String> {
        let now = wisp_common::time::now_ms();
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let mut unique = slug.to_owned();
        let mut n = 2;
        while tx
            .query_row(
                "SELECT 1 FROM skills WHERE slug = ?1",
                [&unique],
                |_| Ok(()),
            )
            .optional()?
            .is_some()
        {
            unique = format!("{slug}-{n}");
            n += 1;
        }
        let id = wisp_common::ids::new_id();
        let vid = wisp_common::ids::new_id();
        tx.execute(
            "INSERT INTO skills (id, slug, name, description, source, enabled, current_version_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![id, unique, name, description, source_str(source), enabled as i64, vid, now],
        )?;
        tx.execute(
            "INSERT INTO skill_versions (id, skill_id, version, content, sha256, created_at) VALUES (?1, ?2, 1, ?3, ?4, ?5)",
            params![vid, id, content, sha(content), now],
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Store new content as the next version (no-op if it's unchanged). Returns the current version.
    pub fn add_skill_version(
        &self,
        id: &str,
        name: &str,
        description: &str,
        content: &str,
    ) -> DbResult<u32> {
        let now = wisp_common::time::now_ms();
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let (current, current_sha): (u32, String) = tx.query_row(
            "SELECT v.version, v.sha256 FROM skills s JOIN skill_versions v ON v.id = s.current_version_id WHERE s.id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if current_sha == sha(content) {
            return Ok(current);
        }
        let next: u32 = tx.query_row(
            "SELECT MAX(version) + 1 FROM skill_versions WHERE skill_id = ?1",
            [id],
            |r| r.get(0),
        )?;
        let vid = wisp_common::ids::new_id();
        tx.execute(
            "INSERT INTO skill_versions (id, skill_id, version, content, sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![vid, id, next, content, sha(content), now],
        )?;
        tx.execute(
            "UPDATE skills SET name = ?2, description = ?3, current_version_id = ?4, updated_at = ?5 WHERE id = ?1",
            params![id, name, description, vid, now],
        )?;
        tx.commit()?;
        Ok(next)
    }

    pub fn set_skill_enabled(&self, id: &str, enabled: bool) -> DbResult<bool> {
        Ok(self.lock()?.execute(
            "UPDATE skills SET enabled = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, enabled as i64, wisp_common::time::now_ms()],
        )? > 0)
    }

    pub fn delete_skill(&self, id: &str) -> DbResult<bool> {
        Ok(self
            .lock()?
            .execute("DELETE FROM skills WHERE id = ?1", [id])?
            > 0)
    }

    /// Newest first.
    pub fn skill_versions(&self, id: &str) -> DbResult<Vec<SkillVersion>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT version, sha256, created_at FROM skill_versions WHERE skill_id = ?1 ORDER BY version DESC")?;
        Ok(stmt
            .query_map([id], |r| {
                Ok(SkillVersion {
                    version: r.get(0)?,
                    sha256: r.get(1)?,
                    created_at_ms: r.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn skill_version_content(&self, id: &str, version: u32) -> DbResult<Option<String>> {
        Ok(self
            .lock()?
            .query_row(
                "SELECT content FROM skill_versions WHERE skill_id = ?1 AND version = ?2",
                params![id, version],
                |r| r.get(0),
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_slugs_and_toggles() {
        let dir = tempfile::tempdir().expect("tmp");
        let db = Database::open(&dir.path().join("t.sqlite3")).expect("open");
        let a = db
            .create_skill("docker", "Docker", "d", SkillSource::User, true, "v1")
            .expect("create");
        let b = db
            .create_skill("docker", "Docker 2", "d", SkillSource::Imported, false, "x")
            .expect("create");
        assert_eq!(db.get_skill(&b).expect("get").expect("b").slug, "docker-2");
        assert_eq!(
            db.add_skill_version(&a, "Docker", "d", "v1").expect("same"),
            1,
            "unchanged content isn't a new version"
        );
        assert_eq!(
            db.add_skill_version(&a, "Docker!", "d2", "v2")
                .expect("edit"),
            2
        );
        let s = db.get_skill(&a).expect("get").expect("a");
        assert_eq!(
            (s.name.as_str(), s.version, s.content.as_str()),
            ("Docker!", 2, "v2")
        );
        assert_eq!(db.skill_versions(&a).expect("versions").len(), 2);
        assert_eq!(
            db.skill_version_content(&a, 1).expect("v1").as_deref(),
            Some("v1")
        );
        db.set_skill_enabled(&a, false).expect("toggle");
        assert!(!db.get_skill(&a).expect("get").expect("a").enabled);
        assert!(db.delete_skill(&a).expect("delete"));
        assert_eq!(db.list_skills().expect("list").len(), 1);
    }
}
