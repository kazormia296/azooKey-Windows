use std::fs;

use shared::ime::{
    ActiveSnapshot, CompositionGenerationPin, ConsumerRegistrar, SnapshotLoadStatus,
    SnapshotLoader, SnapshotRevision,
};

fn fixture_root(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("grimodex-ime-{name}-"))
        .tempdir()
        .expect("create fixture root")
}

fn install_snapshot(root: &std::path::Path, active_project_id: &str, surface: &str) {
    fs::create_dir_all(root.join("projects")).expect("create projects directory");
    fs::create_dir_all(root.join("consumers")).expect("create consumers directory");
    fs::write(
        root.join("state.json"),
        format!(
            r#"{{"format_version":1,"active_project_id":"{active_project_id}","updated_at":"2026-07-12T00:00:00.000Z"}}"#
        ),
    )
    .expect("write state");
    fs::write(
        root.join("projects").join(format!("{active_project_id}.json")),
        format!(
            r#"{{"format_version":1,"project_id":"{active_project_id}","project_name":"星海年代記","generated_at":"2026-07-12T00:00:00.000Z","entries":[{{"yomi":"りゅうせいこう","surface":"{surface}","category":"place","priority":2,"entry_id":"entry-port"}}],"zenzai_context":{{"topic":"溶鉄の星・軍事SF","style":"12345678901234567890123456","preference":"abcdefghijklmnopqrstuvwxyz"}}}}"#
        ),
    )
    .expect("write project");
}

#[test]
fn loader_maps_a_valid_snapshot_and_topics() {
    let root = fixture_root("loader");
    install_snapshot(root.path(), "project-1", "龍星港");

    let result = SnapshotLoader::new(root.path().to_path_buf()).load();

    assert_eq!(result.status, SnapshotLoadStatus::Loaded);
    let snapshot = result.snapshot.expect("loaded snapshot");
    assert_eq!(snapshot.project_id, "project-1");
    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries[0].ruby, "リュウセイコウ");
    assert_eq!(snapshot.entries[0].word, "龍星港");
    assert_eq!(snapshot.entries[0].cid, 1293);
    assert_eq!(snapshot.entries[0].mid, 501);
    assert_eq!(snapshot.entries[0].value, -6.0);
    assert_eq!(snapshot.topic.as_deref(), Some("溶鉄の星・軍事SF"));
    assert_eq!(snapshot.style.as_deref(), Some("1234567890123456789012345"));
    assert_eq!(
        snapshot.preference.as_deref(),
        Some("abcdefghijklmnopqrstuvwxy")
    );
}

#[test]
fn loader_uses_profile_as_topic_when_explicit_context_is_absent() {
    let root = fixture_root("profile");
    fs::create_dir_all(root.path().join("projects")).expect("create projects directory");
    fs::write(
        root.path().join("state.json"),
        r#"{"format_version":1,"active_project_id":"project-1","updated_at":"2026-07-12T00:00:00.000Z"}"#,
    )
    .expect("write state");
    fs::write(
        root.path().join("projects/project-1.json"),
        r#"{"format_version":1,"project_id":"project-1","project_name":"星海年代記","generated_at":"2026-07-12T00:00:00.000Z","entries":[],"profile":"軍事SF。宇宙植民地を舞台にした物語。主要人物: 刹那"}"#,
    )
    .expect("write project");

    let result = SnapshotLoader::new(root.path().to_path_buf()).load();

    assert_eq!(result.status, SnapshotLoadStatus::Loaded);
    assert_eq!(
        result.snapshot.expect("loaded snapshot").topic.as_deref(),
        Some("軍事SF。宇宙植民地を舞台にした物語。主要人物: ")
    );
}

#[test]
fn loader_deduplicates_by_priority_then_entry_id() {
    let root = fixture_root("dedup");
    fs::create_dir_all(root.path().join("projects")).expect("create projects directory");
    fs::write(
        root.path().join("state.json"),
        r#"{"format_version":1,"active_project_id":"project-1","updated_at":"2026-07-12T00:00:00.000Z"}"#,
    )
    .expect("write state");
    fs::write(
        root.path().join("projects/project-1.json"),
        r#"{"format_version":1,"project_id":"project-1","project_name":"星海年代記","generated_at":"2026-07-12T00:00:00.000Z","entries":[{"yomi":"りゅうせいこう","surface":"龍星港","category":"place","priority":1,"entry_id":"z-entry"},{"yomi":"リュウセイコウ","surface":"龍星港","category":"place","priority":3,"entry_id":"low-entry"},{"yomi":"りゅうせいこう","surface":"龍星港","category":"place","priority":3,"entry_id":"a-entry"}]}"#,
    )
    .expect("write project");

    let result = SnapshotLoader::new(root.path().to_path_buf()).load();

    let entry = &result.snapshot.expect("loaded snapshot").entries[0];
    assert_eq!(entry.entry_id, "a-entry");
    assert_eq!(entry.value, -5.0);
}

#[test]
fn loader_is_fail_closed_for_invalid_json_and_inactive_state() {
    let root = fixture_root("invalid");
    fs::write(root.path().join("state.json"), "not-json").expect("write invalid state");
    let invalid = SnapshotLoader::new(root.path().to_path_buf()).load();
    assert_eq!(invalid.status, SnapshotLoadStatus::InvalidState);
    assert!(invalid.snapshot.is_none());

    fs::write(
        root.path().join("state.json"),
        r#"{"format_version":1,"active_project_id":null,"updated_at":"2026-07-12T00:00:00.000Z"}"#,
    )
    .expect("write inactive state");
    let inactive = SnapshotLoader::new(root.path().to_path_buf()).load();
    assert_eq!(inactive.status, SnapshotLoadStatus::Inactive);
    assert!(inactive.snapshot.is_none());
}

#[test]
fn generation_pin_defers_updates_until_composition_boundary() {
    let first = SnapshotRevision::new(1, Some(ActiveSnapshot::empty("project-1")));
    let second = SnapshotRevision::new(2, Some(ActiveSnapshot::empty("project-2")));
    let mut pin = CompositionGenerationPin::default();

    assert!(pin.observe(first.clone()).is_some());
    assert_eq!(pin.begin_composition(first), None);
    assert_eq!(pin.observe(second.clone()), None);
    assert_eq!(pin.pinned_generation(), Some(1));
    assert_eq!(
        pin.end_composition(second)
            .map(|revision| revision.generation),
        Some(2)
    );
    assert_eq!(pin.pinned_generation(), None);
}

#[test]
fn consumer_registrar_writes_and_refreshes_a_canonical_handshake() {
    let root = fixture_root("consumer");
    let registrar = ConsumerRegistrar::new(root.path().to_path_buf(), "1.2.3");

    registrar.register().expect("register consumer");
    let first = fs::read_to_string(root.path().join("consumers/azookey-grimodex.json"))
        .expect("read handshake");
    assert!(first.contains(r#""consumer_id":"azookey-grimodex"#));
    assert!(first.contains(r#""platform":"windows"#));

    std::thread::sleep(std::time::Duration::from_millis(2));
    registrar.heartbeat().expect("refresh consumer");
    let second = fs::read_to_string(root.path().join("consumers/azookey-grimodex.json"))
        .expect("read refreshed handshake");
    assert_ne!(first, second);
}
