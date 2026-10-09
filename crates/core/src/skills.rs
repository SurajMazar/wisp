//! Skills service (spec §64): SKILL.md files kept in the database (versioned) and mirrored to
//! `<data>/skills/<slug>/SKILL.md`. Enabled skills are offered to the model when relevant —
//! never executed. Imported skills start disabled.

use std::path::PathBuf;

use wisp_agents::skills::{self, ParsedSkill};
use wisp_database::repos::skills::StoredSkill;
use wisp_ipc::{
    ErrorCode, IpcError,
    types::{
        app::Empty,
        skills::{Skill, SkillExport, SkillList, SkillSource, SkillSummary, SkillVersions},
    },
};

use crate::Core;

/// Total characters of skill text added to one prompt.
const SKILL_BUDGET: usize = 6_000;
const MAX_ACTIVE: usize = 2;

fn not_found() -> IpcError {
    IpcError::new(ErrorCode::NotFound, "That skill no longer exists.")
}

fn summary(s: &StoredSkill) -> SkillSummary {
    let parsed = skills::parse(&s.content).ok();
    SkillSummary {
        id: s.id.clone(),
        slug: s.slug.clone(),
        name: s.name.clone(),
        description: s.description.clone(),
        source: s.source,
        enabled: s.enabled,
        activation: parsed
            .as_ref()
            .map_or("manual", |p| p.activation.as_str())
            .into(),
        triggers: parsed.map(|p| p.triggers).unwrap_or_default(),
        version: s.version,
        updated_at_ms: s.updated_at_ms,
    }
}

fn full(s: StoredSkill) -> Skill {
    Skill {
        summary: summary(&s),
        content: s.content,
    }
}

/// Only lowercase letters, digits and dashes: safe as a folder name.
fn safe_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 64
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl Core {
    fn skills_dir(&self) -> PathBuf {
        self.data_dir().join("skills")
    }

    fn mirror_skill(&self, slug: &str, content: &str) {
        if !safe_slug(slug) {
            return;
        }
        let dir = self.skills_dir().join(slug);
        if let Err(e) = std::fs::create_dir_all(&dir)
            .and_then(|_| std::fs::write(dir.join("SKILL.md"), content))
        {
            tracing::warn!(error = %e, "couldn't write SKILL.md");
        }
    }

    /// Seed the sample skill and pick up SKILL.md folders added on disk (imported, disabled).
    pub(crate) fn startup_skills(&self) {
        let db = self.db();
        if let Ok(false) = db.skill_slug_exists("mac-keyboard-shortcuts")
            && let Ok(p) = skills::parse(skills::SAMPLE_SKILL)
            && db
                .create_skill(
                    &p.slug,
                    &p.name,
                    &p.description,
                    SkillSource::Builtin,
                    true,
                    skills::SAMPLE_SKILL,
                )
                .is_ok()
        {
            self.mirror_skill(&p.slug, skills::SAMPLE_SKILL);
        }
        let Ok(entries) = std::fs::read_dir(self.skills_dir()) else {
            return;
        };
        for e in entries.flatten() {
            let path = e.path().join("SKILL.md");
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            // Never follow links out of the skills folder; skip oversized files.
            if !meta.is_file() || meta.len() as usize > skills::MAX_SKILL_BYTES {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            match skills::parse(&content) {
                Ok(p) if matches!(db.skill_slug_exists(&p.slug), Ok(false)) => {
                    let _ = db.create_skill(
                        &p.slug,
                        &p.name,
                        &p.description,
                        SkillSource::Imported,
                        false,
                        &content,
                    );
                    tracing::info!(slug = %p.slug, "found a skill on disk (disabled until enabled)");
                }
                Ok(_) => {}
                Err(err) => {
                    tracing::warn!(path = %path.display(), error = %err, "skipping invalid SKILL.md")
                }
            }
        }
    }

    pub(crate) async fn skill_list(&self) -> Result<SkillList, IpcError> {
        let all = self.blocking(|db| db.list_skills()).await?;
        Ok(SkillList {
            skills: all.iter().map(summary).collect(),
        })
    }

    async fn stored(&self, id: &str) -> Result<StoredSkill, IpcError> {
        if !wisp_common::ids::is_valid_id(id) {
            return Err(IpcError::invalid_params("invalid id"));
        }
        let id = id.to_owned();
        self.blocking(move |db| db.get_skill(&id))
            .await?
            .ok_or_else(not_found)
    }

    pub(crate) async fn skill_get(&self, id: &str) -> Result<Skill, IpcError> {
        Ok(full(self.stored(id).await?))
    }

    fn parse_or_explain(content: &str) -> Result<ParsedSkill, IpcError> {
        skills::parse(content).map_err(|e| IpcError::new(ErrorCode::InvalidParams, &e))
    }

    pub(crate) async fn skill_save(
        &self,
        id: Option<String>,
        content: String,
    ) -> Result<Skill, IpcError> {
        let p = Self::parse_or_explain(&content)?;
        let id = match id {
            Some(id) => {
                let existing = self.stored(&id).await?;
                let (i, c) = (id.clone(), content.clone());
                let (n, d) = (p.name.clone(), p.description.clone());
                self.blocking(move |db| db.add_skill_version(&i, &n, &d, &c))
                    .await?;
                self.mirror_skill(&existing.slug, &content);
                id
            }
            None => {
                let c = content.clone();
                let id = self
                    .blocking(move |db| {
                        db.create_skill(
                            &p.slug,
                            &p.name,
                            &p.description,
                            SkillSource::User,
                            true,
                            &c,
                        )
                    })
                    .await?;
                let s = self.stored(&id).await?;
                self.mirror_skill(&s.slug, &content);
                id
            }
        };
        self.skill_get(&id).await
    }

    pub(crate) async fn skill_import(&self, content: String) -> Result<Skill, IpcError> {
        let p = Self::parse_or_explain(&content)?;
        let c = content.clone();
        let id = self
            .blocking(move |db| {
                db.create_skill(
                    &p.slug,
                    &p.name,
                    &p.description,
                    SkillSource::Imported,
                    false,
                    &c,
                )
            })
            .await?;
        let s = self.stored(&id).await?;
        self.mirror_skill(&s.slug, &content);
        Ok(full(s))
    }

    pub(crate) async fn skill_set_enabled(
        &self,
        id: String,
        enabled: bool,
    ) -> Result<SkillSummary, IpcError> {
        self.stored(&id).await?;
        let i = id.clone();
        self.blocking(move |db| db.set_skill_enabled(&i, enabled))
            .await?;
        Ok(summary(&self.stored(&id).await?))
    }

    pub(crate) async fn skill_delete(&self, id: String) -> Result<Empty, IpcError> {
        let s = self.stored(&id).await?;
        self.blocking(move |db| db.delete_skill(&id)).await?;
        if safe_slug(&s.slug) {
            let _ = std::fs::remove_dir_all(self.skills_dir().join(&s.slug));
        }
        Ok(Empty {})
    }

    pub(crate) async fn skill_export(&self, id: String) -> Result<SkillExport, IpcError> {
        let s = self.stored(&id).await?;
        Ok(SkillExport {
            filename: format!("{}.SKILL.md", s.slug),
            content: s.content,
        })
    }

    pub(crate) async fn skill_versions(&self, id: String) -> Result<SkillVersions, IpcError> {
        self.stored(&id).await?;
        Ok(SkillVersions {
            versions: self.blocking(move |db| db.skill_versions(&id)).await?,
        })
    }

    pub(crate) async fn skill_restore(&self, id: String, version: u32) -> Result<Skill, IpcError> {
        let i = id.clone();
        let content = self
            .blocking(move |db| db.skill_version_content(&i, version))
            .await?
            .ok_or_else(|| IpcError::new(ErrorCode::NotFound, "That version doesn't exist."))?;
        self.skill_save(Some(id), content).await
    }

    /// Enabled skills to include for `request`: the attached ones, then the most relevant, within
    /// a character budget. Returns (names, prompt section).
    pub(crate) async fn active_skills(
        &self,
        request: &str,
        attached: &[String],
    ) -> (Vec<String>, Option<String>) {
        let Ok(all) = self.blocking(|db| db.list_skills()).await else {
            return (vec![], None);
        };
        let mut scored: Vec<(u32, ParsedSkill)> = all
            .iter()
            .filter(|s| s.enabled)
            .filter_map(|s| {
                let p = skills::parse(&s.content).ok()?;
                let score = if attached.contains(&s.id) {
                    1_000
                } else {
                    skills::relevance(&p, request)
                };
                (score > 0).then_some((score, p))
            })
            .collect();
        scored.sort_by_key(|s| std::cmp::Reverse(s.0));
        let mut names = vec![];
        let mut section = String::from("Skills you can use (follow them when they apply):");
        let mut room = SKILL_BUDGET;
        for (_, p) in scored.into_iter().take(MAX_ACTIVE.max(attached.len())) {
            if room < 200 {
                break;
            }
            let body: String = p.body.chars().take(room - 100).collect();
            room = room.saturating_sub(body.len() + p.name.len() + 8);
            section.push_str(&format!("\n\n## {}\n{}", p.name, body));
            names.push(p.name);
        }
        if names.is_empty() {
            (vec![], None)
        } else {
            (names, Some(section))
        }
    }
}
