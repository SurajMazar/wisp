//! Settings repository: one JSON row per section.

use rusqlite::params;
use serde_json::Value;
use wisp_ipc::types::settings::{AppSettings, SettingsSection};

use crate::{Database, DbError, DbResult};

impl Database {
    /// Load all settings. Missing sections take defaults. A corrupt section is logged and
    /// replaced by defaults rather than preventing the app from starting.
    pub fn load_settings(&self) -> DbResult<AppSettings> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;

        let mut doc = serde_json::Map::new();
        for row in rows {
            let (key, raw) = row?;
            match serde_json::from_str::<Value>(&raw) {
                Ok(v) => {
                    doc.insert(key, v);
                }
                Err(e) => tracing::warn!(key, error = %e, "ignoring unparsable settings row"),
            }
        }

        // Validate section by section so one bad section doesn't reset the others.
        let mut good = serde_json::Map::new();
        for (key, value) in doc {
            let mut probe = serde_json::Map::new();
            probe.insert(key.clone(), value.clone());
            match serde_json::from_value::<AppSettings>(Value::Object(probe)) {
                Ok(_) => {
                    good.insert(key, value);
                }
                Err(source) => {
                    let err = DbError::InvalidStoredValue {
                        key: key.clone(),
                        source,
                    };
                    tracing::warn!(key, error = %err, "using defaults for settings section");
                }
            }
        }
        let settings: AppSettings = serde_json::from_value(Value::Object(good))?;
        Ok(settings)
    }

    /// Replace one section and return the full, updated settings.
    pub fn save_settings_section(&self, section: &SettingsSection) -> DbResult<AppSettings> {
        // Adjacently tagged: {"section": key, "value": {...}} — store just the value.
        let value = match serde_json::to_value(section)? {
            Value::Object(mut o) => o.remove("value").unwrap_or(Value::Null),
            _ => Value::Null,
        };
        let value = serde_json::to_string(&value)?;
        self.lock()?.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![section.key(), value, wisp_common::time::now_ms()],
        )?;
        self.load_settings()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_ipc::types::settings::*;

    #[test]
    fn round_trips_sections_and_survives_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("s.sqlite3");
        let db = Database::open(&path).expect("open");
        assert_eq!(db.load_settings().expect("load"), AppSettings::default());

        let updated = db
            .save_settings_section(&SettingsSection::Appearance(AppearanceSettings {
                theme: ThemePreference::Dark,
                accent: AccentColor::Cyan,
                custom_color: None,
            }))
            .expect("save");
        assert_eq!(updated.appearance.theme, ThemePreference::Dark);
        drop(db);

        let db = Database::open(&path).expect("reopen");
        let s = db.load_settings().expect("load");
        assert_eq!(s.appearance.accent, AccentColor::Cyan);
        assert_eq!(s.privacy.mode, PrivacyMode::LocalOnly);
    }

    #[test]
    fn corrupt_section_falls_back_to_defaults_without_losing_others() {
        let db = Database::open_in_memory().expect("open");
        db.save_settings_section(&SettingsSection::Privacy(PrivacySettings {
            mode: PrivacyMode::CloudAllowed,
        }))
        .expect("save");
        db.lock()
            .expect("lock")
            .execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('appearance', '{\"theme\":\"neon\"}', 0)",
                [],
            )
            .expect("insert");
        let s = db.load_settings().expect("load");
        assert_eq!(s.appearance, AppearanceSettings::default());
        assert_eq!(s.privacy.mode, PrivacyMode::CloudAllowed);
    }
}
