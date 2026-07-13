use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

#[derive(Deserialize)]
struct ContractLock {
    contract_version: String,
    files: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct Scenario {
    contract_version: String,
    scenario_id: String,
    actions: Vec<serde_json::Value>,
    statuses: Vec<String>,
    snapshots: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct GapMatrix {
    contract_version: String,
    platform: String,
    scenarios: BTreeMap<String, Gap>,
}

#[derive(Deserialize)]
struct Gap {
    status: String,
    gap: String,
    target_slice: u8,
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/composition-behavior-v1")
}

#[test]
fn windows_baseline_pins_every_shared_scenario_and_gap() {
    let expected_scenarios = BTreeSet::from([
        "composing-basic",
        "cursor-editing",
        "escape-backspace",
        "partial-commit",
        "secure-input",
        "segment-editing",
        "server-failure",
        "stale-candidate",
        "unicode-caret",
    ]);
    let root = fixture_root();
    let lock: ContractLock = serde_json::from_slice(
        &fs::read(root.join("contract-lock.json")).expect("read contract lock"),
    )
    .expect("decode contract lock");
    let matrix: GapMatrix =
        serde_json::from_slice(&fs::read(root.join("gap-matrix.json")).expect("read gap matrix"))
            .expect("decode gap matrix");
    assert_eq!(lock.contract_version, "composition-behavior-v1");
    assert_eq!(matrix.contract_version, lock.contract_version);
    assert_eq!(matrix.platform, "windows-tsf");

    let mut observed = BTreeMap::new();
    for (filename, expected_hash) in &lock.files {
        let data = fs::read(root.join("scenarios").join(filename)).expect("read scenario");
        let actual_hash = format!("{:x}", Sha256::digest(&data));
        assert_eq!(&actual_hash, expected_hash, "fixture drift: {filename}");
        let scenario: Scenario = serde_json::from_slice(&data).expect("decode scenario");
        assert_eq!(scenario.contract_version, lock.contract_version);
        assert_eq!(filename, &format!("{}.json", scenario.scenario_id));
        assert!(
            !scenario.actions.is_empty(),
            "empty action trace: {filename}"
        );
        assert_eq!(scenario.actions.len(), scenario.statuses.len());
        assert_eq!(scenario.actions.len(), scenario.snapshots.len());
        assert!(scenario.actions.iter().all(|action| {
            action
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| !value.is_empty())
        }));
        assert!(scenario.snapshots.iter().all(serde_json::Value::is_object));
        assert!(scenario.statuses.iter().all(|status| matches!(
            status.as_str(),
            "success"
                | "stale_revision"
                | "stale_candidate"
                | "invalid_action"
                | "converter_unavailable"
                | "secure_input_violation"
        )));
        observed.insert(scenario.scenario_id, ());
    }

    assert_eq!(
        observed.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        expected_scenarios
    );
    assert_eq!(
        observed.keys().collect::<Vec<_>>(),
        matrix.scenarios.keys().collect::<Vec<_>>()
    );
    for (scenario_id, gap) in &matrix.scenarios {
        assert!(matches!(gap.status.as_str(), "partial" | "gap"));
        assert!(
            !gap.gap.trim().is_empty(),
            "missing gap reason: {scenario_id}"
        );
        assert!((1..=5).contains(&gap.target_slice));
    }
}
