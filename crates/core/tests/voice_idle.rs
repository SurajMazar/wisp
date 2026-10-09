#![allow(clippy::expect_used)] // test harness

//! Idle speech models are released (and nothing happens when none were loaded).

use wisp_common::paths::DataPaths;
use wisp_core::{Core, events::EventBus};
use wisp_database::Database;

#[tokio::test]
async fn unloading_with_nothing_loaded_is_a_no_op() {
    let dir = tempfile::tempdir().expect("tmp");
    let paths = DataPaths::new(dir.path());
    paths.ensure().expect("dirs");
    let core = Core::new(
        Database::open(&paths.database_file()).expect("db"),
        &paths,
        EventBus::new(),
    )
    .expect("core");
    assert!(!core.unload_idle_voice(std::time::Duration::ZERO));
    assert_eq!(wisp_core::voice::VOICE_IDLE.as_secs(), 900);
}
