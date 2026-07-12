use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use unicode_normalization::UnicodeNormalization;

pub const CONSUMER_ID: &str = "azookey-grimodex";
pub const CONSUMER_NAME: &str = "Grimodex IME for Windows";
pub const CONSUMER_PLATFORM: &str = "windows";
pub const CONSUMER_HEARTBEAT: Duration = Duration::from_secs(900);

const STATE_MAX_BYTES: usize = 65_536;
const PROJECT_MAX_BYTES: usize = 16_777_216;
const CONSUMER_MAX_BYTES: usize = 65_536;
const PROJECT_MAX_ENTRIES: usize = 20_000;
const PROJECT_ID_MAX_CHARS: usize = 128;
const PROJECT_NAME_MAX_CHARS: usize = 256;
const ENTRY_YOMI_MAX_CHARS: usize = 256;
const ENTRY_SURFACE_MAX_CHARS: usize = 256;
const ENTRY_ID_MAX_CHARS: usize = 128;
const PROFILE_MAX_CHARS: usize = 400;
const ZENZAI_CONDITION_MAX_CHARS: usize = 200;
const CONVERTER_CONDITION_MAX_CHARS: usize = 25;
const TIMESTAMP_MAX_CHARS: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MappedDictionaryEntry {
    pub ruby: String,
    pub word: String,
    pub cid: i32,
    pub mid: i32,
    pub value: f32,
    pub entry_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActiveSnapshot {
    pub project_id: String,
    pub project_name: String,
    pub entries: Vec<MappedDictionaryEntry>,
    pub topic: Option<String>,
    pub style: Option<String>,
    pub preference: Option<String>,
}

impl ActiveSnapshot {
    pub fn empty(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            project_name: String::new(),
            entries: Vec::new(),
            topic: None,
            style: None,
            preference: None,
        }
    }

    pub fn converter_payload(&self) -> ConverterPayload<'_> {
        ConverterPayload {
            entries: &self.entries,
            topic: self.topic.as_deref(),
            style: self.style.as_deref(),
            preference: self.preference.as_deref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConverterPayload<'a> {
    pub entries: &'a [MappedDictionaryEntry],
    pub topic: Option<&'a str>,
    pub style: Option<&'a str>,
    pub preference: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotLoadStatus {
    Loaded,
    Inactive,
    MissingState,
    MissingSnapshot,
    InvalidState,
    InvalidSnapshot,
    StateChangedDuringRead,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotLoadResult {
    pub snapshot: Option<ActiveSnapshot>,
    pub status: SnapshotLoadStatus,
}

#[derive(Debug, Clone)]
pub struct SnapshotLoader {
    root: PathBuf,
}

impl SnapshotLoader {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn load(&self) -> SnapshotLoadResult {
        let state_path = self.root.join("state.json");
        let first_state = match read_bounded(&state_path, STATE_MAX_BYTES) {
            Ok(Some(bytes)) => match parse_state(&bytes) {
                Ok(state) => state,
                Err(()) => return invalid(SnapshotLoadStatus::InvalidState),
            },
            Ok(None) => return invalid(SnapshotLoadStatus::MissingState),
            Err(_) => return invalid(SnapshotLoadStatus::InvalidState),
        };

        let Some(project_id) = first_state.active_project_id.as_deref() else {
            return invalid(SnapshotLoadStatus::Inactive);
        };
        let project_path = self
            .root
            .join("projects")
            .join(format!("{project_id}.json"));
        let project = match read_bounded(&project_path, PROJECT_MAX_BYTES) {
            Ok(Some(bytes)) => match parse_project(&bytes, project_id) {
                Ok(project) => project,
                Err(()) => return invalid(SnapshotLoadStatus::InvalidSnapshot),
            },
            Ok(None) => return invalid(SnapshotLoadStatus::MissingSnapshot),
            Err(_) => return invalid(SnapshotLoadStatus::InvalidSnapshot),
        };

        let second_state = match read_bounded(&state_path, STATE_MAX_BYTES) {
            Ok(Some(bytes)) => match parse_state(&bytes) {
                Ok(state) => state,
                Err(()) => return invalid(SnapshotLoadStatus::StateChangedDuringRead),
            },
            Ok(None) | Err(_) => return invalid(SnapshotLoadStatus::StateChangedDuringRead),
        };
        if first_state.active_project_id != second_state.active_project_id {
            return invalid(SnapshotLoadStatus::StateChangedDuringRead);
        }

        SnapshotLoadResult {
            snapshot: Some(map_project(project)),
            status: SnapshotLoadStatus::Loaded,
        }
    }
}

fn invalid(status: SnapshotLoadStatus) -> SnapshotLoadResult {
    SnapshotLoadResult {
        snapshot: None,
        status,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WireState {
    format_version: u64,
    active_project_id: Option<String>,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct WireProject {
    format_version: u64,
    project_id: String,
    project_name: String,
    generated_at: String,
    entries: Vec<WireEntry>,
    profile: Option<String>,
    zenzai_context: Option<WireZenzaiContext>,
}

#[derive(Debug, Deserialize)]
struct WireEntry {
    yomi: String,
    surface: String,
    category: WireCategory,
    priority: u8,
    entry_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum WireCategory {
    Person,
    Place,
    Noun,
}

#[derive(Debug, Deserialize)]
struct WireZenzaiContext {
    topic: String,
    style: Option<String>,
    preference: Option<String>,
}

fn read_bounded(path: &Path, max_bytes: usize) -> io::Result<Option<Vec<u8>>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if file.metadata()?.len() > max_bytes as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} exceeds {max_bytes} bytes", path.display()),
        ));
    }
    let mut bytes = Vec::with_capacity(max_bytes.min(64 * 1024));
    file.take((max_bytes + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} exceeds {max_bytes} bytes", path.display()),
        ));
    }
    Ok(Some(bytes))
}

fn parse_state(bytes: &[u8]) -> Result<WireState, ()> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| ())?;
    let object = value.as_object().ok_or(())?;
    if !object.contains_key("format_version")
        || !object.contains_key("active_project_id")
        || !object.contains_key("updated_at")
    {
        return Err(());
    }
    let state = WireState {
        format_version: object
            .get("format_version")
            .and_then(Value::as_u64)
            .ok_or(())?,
        active_project_id: match object.get("active_project_id") {
            Some(Value::Null) => None,
            Some(Value::String(value)) => Some(value.clone()),
            _ => return Err(()),
        },
        updated_at: object
            .get("updated_at")
            .and_then(Value::as_str)
            .ok_or(())?
            .to_string(),
    };
    if state.format_version == 1
        && valid_timestamp(&state.updated_at)
        && state
            .active_project_id
            .as_deref()
            .map(valid_project_id)
            .unwrap_or(true)
    {
        Ok(state)
    } else {
        Err(())
    }
}

fn parse_project(bytes: &[u8], expected_project_id: &str) -> Result<WireProject, ()> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| ())?;
    let object = value.as_object().ok_or(())?;
    for key in [
        "format_version",
        "project_id",
        "project_name",
        "generated_at",
        "entries",
    ] {
        if !object.contains_key(key) {
            return Err(());
        }
    }
    if let Some(profile) = object.get("profile") {
        if !profile.is_string() {
            return Err(());
        }
    }
    if let Some(context) = object.get("zenzai_context") {
        let context = context.as_object().ok_or(())?;
        for key in ["topic", "style", "preference"] {
            if !context.contains_key(key) {
                return Err(());
            }
        }
    }
    let project: WireProject = match serde_json::from_value(value) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("project decode failed: {error}");
            return Err(());
        }
    };
    if project.format_version != 1
        || project.project_id != expected_project_id
        || !valid_project_id(&project.project_id)
        || !valid_text(&project.project_name, 1, PROJECT_NAME_MAX_CHARS)
        || !valid_timestamp(&project.generated_at)
        || project.entries.len() > PROJECT_MAX_ENTRIES
        || !project
            .profile
            .as_deref()
            .map(|value| valid_text(value, 0, PROFILE_MAX_CHARS))
            .unwrap_or(true)
    {
        return Err(());
    }
    if let Some(context) = project.zenzai_context.as_ref() {
        if !valid_text(&context.topic, 1, ZENZAI_CONDITION_MAX_CHARS)
            || !context
                .style
                .as_deref()
                .map(|value| valid_text(value, 0, ZENZAI_CONDITION_MAX_CHARS))
                .unwrap_or(true)
            || !context
                .preference
                .as_deref()
                .map(|value| valid_text(value, 0, ZENZAI_CONDITION_MAX_CHARS))
                .unwrap_or(true)
        {
            return Err(());
        }
    }
    if !project.entries.iter().all(|entry| {
        valid_text(&entry.yomi, 1, ENTRY_YOMI_MAX_CHARS)
            && valid_text(&entry.surface, 1, ENTRY_SURFACE_MAX_CHARS)
            && (1..=3).contains(&entry.priority)
            && valid_entry_id(&entry.entry_id)
    }) {
        return Err(());
    }
    Ok(project)
}

fn map_project(project: WireProject) -> ActiveSnapshot {
    let mut mapped = Vec::with_capacity(project.entries.len());
    for entry in project.entries {
        let ruby = to_katakana(&entry.yomi);
        let word: String = entry.surface.nfc().collect();
        let cid = match entry.category {
            WireCategory::Person => 1289,
            WireCategory::Place => 1293,
            WireCategory::Noun => 1288,
        };
        let base = match entry.priority {
            3 => -4.0,
            2 => -5.0,
            _ => -8.0,
        };
        let value = if matches!(entry.category, WireCategory::Person) {
            base
        } else {
            base - 1.0
        };
        mapped.push((
            entry.priority,
            MappedDictionaryEntry {
                ruby,
                word,
                cid,
                mid: 501,
                value,
                entry_id: entry.entry_id,
            },
        ));
    }

    mapped.sort_by(|left, right| {
        right.0.cmp(&left.0).then_with(|| {
            left.1
                .ruby
                .as_bytes()
                .cmp(right.1.ruby.as_bytes())
                .then_with(|| left.1.word.as_bytes().cmp(right.1.word.as_bytes()))
                .then_with(|| left.1.cid.cmp(&right.1.cid))
                .then_with(|| left.1.entry_id.as_bytes().cmp(right.1.entry_id.as_bytes()))
        })
    });

    let mut unique: Vec<(u8, MappedDictionaryEntry)> = Vec::with_capacity(mapped.len());
    let mut indices: HashMap<(String, String, i32), usize> = HashMap::with_capacity(mapped.len());
    for (priority, entry) in mapped {
        let key = (entry.ruby.clone(), entry.word.clone(), entry.cid);
        if let Some(&index) = indices.get(&key) {
            let current = &unique[index];
            if current.0 > priority || current.1.entry_id.as_bytes() <= entry.entry_id.as_bytes() {
                continue;
            }
            unique[index] = (priority, entry);
        } else {
            indices.insert(key, unique.len());
            unique.push((priority, entry));
        }
    }

    let (topic, style, preference) = match project.zenzai_context {
        Some(context) => (
            Some(converter_condition(context.topic)),
            context.style.map(converter_condition),
            context.preference.map(converter_condition),
        ),
        None => (
            project
                .profile
                .filter(|profile| !profile.is_empty())
                .map(converter_condition),
            None,
            None,
        ),
    };

    ActiveSnapshot {
        project_id: project.project_id,
        project_name: project.project_name,
        entries: unique.into_iter().map(|(_, entry)| entry).collect(),
        topic,
        style,
        preference,
    }
}

fn converter_condition(value: String) -> String {
    value.chars().take(CONVERTER_CONDITION_MAX_CHARS).collect()
}

fn to_katakana(value: &str) -> String {
    value
        .nfkc()
        .map(|character| {
            let codepoint = character as u32;
            if (0x3041..=0x3096).contains(&codepoint) {
                char::from_u32(codepoint + 0x60).unwrap_or(character)
            } else {
                character
            }
        })
        .collect()
}

fn valid_project_id(value: &str) -> bool {
    valid_ascii_identifier(value, PROJECT_ID_MAX_CHARS)
}

fn valid_entry_id(value: &str) -> bool {
    valid_ascii_identifier(value, ENTRY_ID_MAX_CHARS)
}

fn valid_ascii_identifier(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.chars().count() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn valid_text(value: &str, minimum: usize, maximum: usize) -> bool {
    let count = value.chars().count();
    count >= minimum
        && count <= maximum
        && value.chars().all(|character| {
            let codepoint = character as u32;
            !(codepoint <= 0x1f || (0x7f..=0x9f).contains(&codepoint))
        })
}

fn valid_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if value.chars().count() != bytes.len()
        || !(20..=TIMESTAMP_MAX_CHARS).contains(&bytes.len())
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
        || !digits(bytes, 0..4)
        || !digits(bytes, 5..7)
        || !digits(bytes, 8..10)
        || !digits(bytes, 11..13)
        || !digits(bytes, 14..16)
        || !digits(bytes, 17..19)
    {
        return false;
    }
    let mut zone_index = 19;
    if bytes.get(zone_index) == Some(&b'.') {
        zone_index += 1;
        let start = zone_index;
        while bytes.get(zone_index).is_some_and(u8::is_ascii_digit) {
            zone_index += 1;
        }
        if !(1..=9).contains(&(zone_index - start)) {
            return false;
        }
    }
    if bytes.get(zone_index) == Some(&b'Z') {
        if zone_index + 1 != bytes.len() {
            return false;
        }
    } else if zone_index + 6 != bytes.len()
        || !matches!(bytes.get(zone_index), Some(b'+' | b'-'))
        || bytes.get(zone_index + 3) != Some(&b':')
        || !digits(bytes, zone_index + 1..zone_index + 3)
        || !digits(bytes, zone_index + 4..zone_index + 6)
        || number(bytes, zone_index + 1..zone_index + 3) > 23
        || number(bytes, zone_index + 4..zone_index + 6) > 59
    {
        return false;
    }
    let year = number(bytes, 0..4);
    let month = number(bytes, 5..7);
    let day = number(bytes, 8..10);
    let hour = number(bytes, 11..13);
    let minute = number(bytes, 14..16);
    let second = number(bytes, 17..19);
    (1..=12).contains(&month)
        && hour <= 23
        && minute <= 59
        && second <= 59
        && (1..=days_in_month(year, month)).contains(&day)
}

fn digits(bytes: &[u8], mut range: std::ops::Range<usize>) -> bool {
    range.all(|index| bytes.get(index).is_some_and(u8::is_ascii_digit))
}

fn number(bytes: &[u8], range: std::ops::Range<usize>) -> u32 {
    range.fold(0, |result, index| {
        result * 10 + u32::from(bytes[index] - b'0')
    })
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotRevision {
    pub generation: u64,
    pub snapshot: Option<ActiveSnapshot>,
}

impl SnapshotRevision {
    pub fn new(generation: u64, snapshot: Option<ActiveSnapshot>) -> Self {
        Self {
            generation,
            snapshot,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct CompositionGenerationPin {
    applied: Option<SnapshotRevision>,
    pending: Option<SnapshotRevision>,
    pinned: Option<SnapshotRevision>,
}

impl CompositionGenerationPin {
    pub fn observe(&mut self, revision: SnapshotRevision) -> Option<SnapshotRevision> {
        let baseline = self
            .pending
            .as_ref()
            .or(self.pinned.as_ref())
            .or(self.applied.as_ref());
        if let Some(baseline) = baseline {
            if revision.generation < baseline.generation || revision == *baseline {
                return None;
            }
        }
        if self.pinned.is_some() {
            self.pending = Some(revision);
            None
        } else {
            self.applied = Some(revision.clone());
            self.pending = None;
            Some(revision)
        }
    }

    pub fn begin_composition(&mut self, latest: SnapshotRevision) -> Option<SnapshotRevision> {
        if self.pinned.is_some() {
            let _ = self.observe(latest);
            return None;
        }
        let applied = self.observe(latest);
        self.pinned = self.applied.clone();
        applied
    }

    pub fn end_composition(&mut self, latest: SnapshotRevision) -> Option<SnapshotRevision> {
        if self.pinned.is_none() {
            return self.observe(latest);
        }
        let _ = self.observe(latest);
        self.pinned = None;
        let Some(next) = self.pending.take() else {
            return None;
        };
        if self.applied.as_ref() == Some(&next) {
            None
        } else {
            self.applied = Some(next.clone());
            Some(next)
        }
    }

    pub fn pinned_generation(&self) -> Option<u64> {
        self.pinned.as_ref().map(|revision| revision.generation)
    }

    pub fn active_revision(&self) -> Option<&SnapshotRevision> {
        self.pinned.as_ref().or(self.applied.as_ref())
    }
}

#[derive(Debug, Clone)]
pub struct ConsumerRegistrar {
    root: PathBuf,
    version: String,
}

impl ConsumerRegistrar {
    pub fn new(root: PathBuf, version: impl Into<String>) -> Self {
        Self {
            root,
            version: version.into(),
        }
    }

    pub fn register(&self) -> io::Result<()> {
        self.write_handshake()
    }

    pub fn heartbeat(&self) -> io::Result<()> {
        self.write_handshake()
    }

    pub fn unregister(&self) -> io::Result<()> {
        match fs::remove_file(self.handshake_path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn handshake_path(&self) -> PathBuf {
        self.root
            .join("consumers")
            .join(format!("{CONSUMER_ID}.json"))
    }

    fn write_handshake(&self) -> io::Result<()> {
        let destination = self.handshake_path();
        let parent = destination
            .parent()
            .ok_or_else(|| io::Error::other("consumer handshake has no parent"))?;
        fs::create_dir_all(parent)?;
        let payload = serde_json::json!({
            "format_version": 1,
            "consumer_id": CONSUMER_ID,
            "name": CONSUMER_NAME,
            "version": self.version,
            "platform": CONSUMER_PLATFORM,
            "capabilities": {
                "profile": true,
                "dynamic_dictionary": true,
                "zenzai_v3_conditions": true,
                "application_scoping": true,
            },
            "last_seen": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        });
        let bytes = serde_json::to_vec(&payload).map_err(io::Error::other)?;
        if bytes.len() > CONSUMER_MAX_BYTES {
            return Err(io::Error::other(
                "consumer handshake exceeds protocol limit",
            ));
        }
        atomic_write(&destination, &bytes)
    }
}

pub fn resolve_root() -> PathBuf {
    std::env::var_os("GRIMODEX_IME_ROOT")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::get_config_root())
}

pub fn allows_application_scope(application_id: &str) -> bool {
    matches!(
        application_id.trim().to_ascii_lowercase().as_str(),
        "grimodex" | "grimodex.exe"
    )
}

fn atomic_write(destination: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::other("atomic destination has no parent"))?;
    let temporary = parent.join(format!(
        ".{}.tmp-{}-{}",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("payload"),
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let mut file = File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    replace_file(&temporary, destination)
}

#[cfg(not(windows))]
fn replace_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temporary, destination)
}

#[cfg(windows)]
fn replace_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let temporary: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe {
        MoveFileExW(
            PCWSTR(temporary.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::{allows_application_scope, to_katakana, valid_timestamp};

    #[test]
    fn normalizes_hiragana_and_nfkc_before_mapping() {
        assert_eq!(to_katakana("りゅうｾｲこう"), "リュウセイコウ");
    }

    #[test]
    fn validates_calendar_timestamps() {
        assert!(valid_timestamp("2026-02-28T12:34:56.000Z"));
        assert!(!valid_timestamp("2026-02-29T12:34:56.000Z"));
        assert!(!valid_timestamp("2026-01-01T12:34:56Z+00:00"));
    }

    #[test]
    fn application_scope_is_fail_closed() {
        assert!(allows_application_scope("Grimodex.exe"));
        assert!(!allows_application_scope("notepad.exe"));
        assert!(!allows_application_scope(""));
    }
}
