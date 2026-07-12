use azookey_server::TonicNamedPipeServer;
use tonic::{transport::Server, Request, Response, Status};
use tonic_reflection::server::Builder as ReflectionBuilder;

use shared::proto::{
    AppendTextRequest, AppendTextResponse, ClearTextRequest, ClearTextResponse,
    CloseSessionRequest, CloseSessionResponse, ComposingText, MoveCursorRequest,
    MoveCursorResponse, OpenSessionRequest, OpenSessionResponse, RemoveTextRequest,
    RemoveTextResponse, ShrinkTextRequest, ShrinkTextResponse, Suggestion,
};
use shared::{
    ime::{
        allows_application_scope, resolve_root, ActiveSnapshot, CompositionGenerationPin,
        ConsumerRegistrar, SnapshotLoader, SnapshotRevision, CONSUMER_HEARTBEAT,
    },
    proto::azookey_service_server::{AzookeyService, AzookeyServiceServer},
    server_pipe_name,
};

use std::{
    collections::HashMap,
    ffi::{c_char, c_int, c_void, CStr, CString},
    path::PathBuf,
    sync::{Arc, Mutex},
};

mod watcher;

const MAX_SESSION_ID_LENGTH: usize = 128;

#[derive(Debug, Clone)]
struct SessionMetadata {
    input_scope: String,
    secure: bool,
    converter_handle: usize,
    integration_allowed: bool,
    generation_pin: CompositionGenerationPin,
}

impl SessionMetadata {
    fn grimodex_integration_allowed(&self) -> bool {
        self.integration_allowed && !self.secure && self.input_scope != "password"
    }
}

#[derive(Debug, Default)]
struct SessionRegistry {
    sessions: Mutex<HashMap<String, SessionMetadata>>,
}

impl SessionRegistry {
    fn validate_id(session_id: &str) -> Result<(), &'static str> {
        if session_id.is_empty() || session_id.len() > MAX_SESSION_ID_LENGTH {
            return Err("invalid session id length");
        }
        if !session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
        {
            return Err("invalid session id characters");
        }
        Ok(())
    }

    fn open(&self, session_id: String, metadata: SessionMetadata) -> Result<(), &'static str> {
        Self::validate_id(&session_id)?;
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "session registry poisoned")?;
        if sessions.contains_key(&session_id) {
            return Err("session already exists");
        }
        sessions.insert(session_id, metadata);
        Ok(())
    }

    fn require(&self, session_id: &str) -> Result<SessionMetadata, &'static str> {
        Self::validate_id(session_id)?;
        self.sessions
            .lock()
            .map_err(|_| "session registry poisoned")?
            .get(session_id)
            .cloned()
            .ok_or("session is not open")
    }

    fn close(&self, session_id: &str) -> Result<SessionMetadata, &'static str> {
        Self::validate_id(session_id)?;
        self.sessions
            .lock()
            .map_err(|_| "session registry poisoned")?
            .remove(session_id)
            .ok_or("session is not open")
    }

    fn with_mut<T>(
        &self,
        session_id: &str,
        operation: impl FnOnce(&mut SessionMetadata) -> Result<T, &'static str>,
    ) -> Result<T, &'static str> {
        Self::validate_id(session_id)?;
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "session registry poisoned")?;
        let session = sessions.get_mut(session_id).ok_or("session is not open")?;
        operation(session)
    }
}

#[derive(Debug)]
struct SnapshotManager {
    loader: SnapshotLoader,
    state: Mutex<(u64, Option<ActiveSnapshot>)>,
}

impl SnapshotManager {
    fn new(root: PathBuf) -> Arc<Self> {
        let manager = Arc::new(Self {
            loader: SnapshotLoader::new(root),
            state: Mutex::new((0, None)),
        });
        manager.reload();
        manager
    }

    fn current(&self) -> SnapshotRevision {
        self.state
            .lock()
            .map(|state| SnapshotRevision::new(state.0, state.1.clone()))
            .unwrap_or_else(|_| SnapshotRevision::new(0, None))
    }

    fn reload(&self) -> SnapshotRevision {
        let next = self.loader.load().snapshot;
        let Ok(mut state) = self.state.lock() else {
            return SnapshotRevision::new(0, None);
        };
        if state.1 != next {
            state.0 = state.0.saturating_add(1);
            state.1 = next;
        }
        SnapshotRevision::new(state.0, state.1.clone())
    }
}

struct RawComposingText {
    text: String,
    _cursor: i8,
}

#[derive(Debug, Clone)]
#[repr(C)]
struct FFICandidate {
    text: *mut c_char,
    subtext: *mut c_char,
    hiragana: *mut c_char,
    corresponding_count: c_int,
}

unsafe extern "C" {
    fn CreateSession(path: *const c_char, use_zenzai: bool) -> *mut c_void;
    fn DestroySession(handle: *mut c_void);
    fn SetContext(handle: *mut c_void, context: *const c_char);
    fn AppendText(handle: *mut c_void, input: *const c_char, cursorPtr: *mut c_int) -> *mut c_char;
    fn RemoveText(handle: *mut c_void, cursorPtr: *mut c_int) -> *mut c_char;
    fn MoveCursor(handle: *mut c_void, offset: c_int, cursorPtr: *mut c_int) -> *mut c_char;
    fn ShrinkText(handle: *mut c_void, offset: c_int) -> *mut c_char;
    fn ClearText(handle: *mut c_void);
    fn GetComposedText(handle: *mut c_void, lengthPtr: *mut c_int) -> *mut *mut FFICandidate;
    fn LoadConfig(handle: *mut c_void);
    fn SetGrimodexPayload(handle: *mut c_void, payload: *const c_char);
}

fn apply_revision(
    handle: *mut c_void,
    integration_allowed: bool,
    revision: &SnapshotRevision,
) -> Result<(), &'static str> {
    let snapshot = if integration_allowed {
        revision.snapshot.clone()
    } else {
        None
    };
    let snapshot = snapshot.unwrap_or_else(|| ActiveSnapshot::empty("inactive"));
    let payload = serde_json::to_string(&snapshot.converter_payload())
        .map_err(|_| "serialize Grimodex converter payload")?;
    let payload = CString::new(payload).map_err(|_| "payload contains NUL")?;
    unsafe { SetGrimodexPayload(handle, payload.as_ptr()) };
    Ok(())
}

fn create_session(path: &str, use_zenzai: bool) -> Result<*mut c_void, Status> {
    let path =
        CString::new(path).map_err(|_| Status::invalid_argument("converter path contains NUL"))?;
    unsafe {
        let handle = CreateSession(path.as_ptr(), use_zenzai);
        if handle.is_null() {
            Err(Status::internal("converter session allocation failed"))
        } else {
            Ok(handle)
        }
    }
}

unsafe fn read_c_string(pointer: *const c_char) -> String {
    if pointer.is_null() {
        return String::new();
    }
    CStr::from_ptr(pointer).to_string_lossy().into_owned()
}

fn add_text(handle: *mut c_void, input: &str) -> RawComposingText {
    unsafe {
        let input = CString::new(input).unwrap_or_default();
        let mut cursor: c_int = 0;

        let result = AppendText(handle, input.as_ptr(), &mut cursor);

        let text = read_c_string(result);

        RawComposingText {
            text,
            _cursor: cursor as i8,
        }
    }
}

fn move_cursor(handle: *mut c_void, offset: i8) -> RawComposingText {
    unsafe {
        let offset = c_int::from(offset);
        println!("Offset: {}", offset);
        let mut cursor: c_int = 0;

        let result = MoveCursor(handle, offset, &mut cursor);

        let text = read_c_string(result);

        RawComposingText {
            text,
            _cursor: cursor as i8,
        }
    }
}

fn remove_text(handle: *mut c_void) -> RawComposingText {
    unsafe {
        let mut cursor: c_int = 0;

        let result = RemoveText(handle, &mut cursor);

        let text = read_c_string(result);

        RawComposingText {
            text,
            _cursor: cursor as i8,
        }
    }
}

fn clear_text(handle: *mut c_void) {
    unsafe {
        ClearText(handle);
    }
}

fn get_composed_text(handle: *mut c_void) -> Vec<Suggestion> {
    unsafe {
        let mut length: c_int = 0;
        let result = GetComposedText(handle, &mut length);
        let mut suggestions = Vec::with_capacity(length as usize);

        for index in 0..length as usize {
            let candidate = (**result.add(index)).clone();
            let text = CStr::from_ptr(candidate.text)
                .to_string_lossy()
                .into_owned();
            let subtext = CStr::from_ptr(candidate.subtext)
                .to_string_lossy()
                .into_owned();
            let corresponding_count = candidate.corresponding_count;

            let suggestion = Suggestion {
                text,
                subtext,
                corresponding_count,
            };

            // check if suggestions have the same text
            if suggestions
                .iter()
                .any(|s: &Suggestion| s.text == suggestion.text)
            {
                continue;
            }
            suggestions.push(suggestion);
        }

        suggestions
    }
}

fn shrink_text(handle: *mut c_void, offset: i8) -> RawComposingText {
    unsafe {
        let offset = c_int::from(offset);
        let result = ShrinkText(handle, offset);

        let text = read_c_string(result);

        RawComposingText { text, _cursor: 0 }
    }
}

#[derive(Debug, Clone)]
pub struct SessionAzookeyService {
    registry: Arc<SessionRegistry>,
    converter_lock: Arc<tokio::sync::Mutex<()>>,
    converter_path: Arc<PathBuf>,
    snapshot_manager: Arc<SnapshotManager>,
}

impl SessionAzookeyService {
    fn new(converter_path: PathBuf, snapshot_root: PathBuf) -> Self {
        Self {
            registry: Arc::new(SessionRegistry::default()),
            converter_lock: Arc::new(tokio::sync::Mutex::new(())),
            converter_path: Arc::new(converter_path),
            snapshot_manager: SnapshotManager::new(snapshot_root),
        }
    }

    fn require_session(&self, session_id: &str) -> Result<SessionMetadata, Status> {
        self.registry
            .require(session_id)
            .map_err(|message| Status::failed_precondition(message))
    }

    fn reload_snapshot_and_sessions(&self) {
        let revision = self.snapshot_manager.reload();
        let Ok(mut sessions) = self.registry.sessions.lock() else {
            eprintln!("Grimodex session registry is poisoned while reloading snapshot");
            return;
        };
        for session in sessions.values_mut() {
            unsafe { LoadConfig(session.converter_handle as *mut c_void) };
            if let Some(applied) = session.generation_pin.observe(revision.clone()) {
                if let Err(error) = apply_revision(
                    session.converter_handle as *mut c_void,
                    session.grimodex_integration_allowed(),
                    &applied,
                ) {
                    eprintln!("Grimodex snapshot apply failed: {error}");
                }
            }
        }
    }
}

#[tonic::async_trait]
impl AzookeyService for SessionAzookeyService {
    async fn open_session(
        &self,
        request: Request<OpenSessionRequest>,
    ) -> Result<Response<OpenSessionResponse>, Status> {
        let request = request.into_inner();
        if request.input_scope.len() > 256 {
            return Err(Status::invalid_argument("input scope is too long"));
        }
        if request.application_id.len() > 128 {
            return Err(Status::invalid_argument("application id is too long"));
        }
        if !matches!(
            request.input_scope.as_str(),
            "text" | "search" | "password" | "control"
        ) {
            return Err(Status::invalid_argument("unsupported input scope"));
        }
        let secure = request.secure || request.input_scope == "password";
        let handle = create_session(
            self.converter_path
                .to_str()
                .ok_or_else(|| Status::internal("converter path is not UTF-8"))?,
            !secure,
        )?;
        let integration_allowed = !secure && allows_application_scope(&request.application_id);
        let current_revision = self.snapshot_manager.current();
        let mut generation_pin = CompositionGenerationPin::default();
        let Some(applied) = generation_pin.observe(current_revision) else {
            unsafe { DestroySession(handle) };
            return Err(Status::internal(
                "initial Grimodex revision was not applied",
            ));
        };
        if let Err(error) = apply_revision(handle, integration_allowed, &applied) {
            unsafe { DestroySession(handle) };
            return Err(Status::internal(error));
        }
        let metadata = SessionMetadata {
            input_scope: request.input_scope,
            secure,
            converter_handle: handle as usize,
            integration_allowed,
            generation_pin,
        };
        if let Err(error) = self.registry.open(request.session_id, metadata) {
            unsafe { DestroySession(handle) };
            return Err(Status::failed_precondition(error));
        }
        Ok(Response::new(OpenSessionResponse {}))
    }

    async fn close_session(
        &self,
        request: Request<CloseSessionRequest>,
    ) -> Result<Response<CloseSessionResponse>, Status> {
        let session_id = request.into_inner().session_id;
        let _guard = self.converter_lock.lock().await;
        let metadata = self
            .registry
            .close(&session_id)
            .map_err(Status::failed_precondition)?;
        unsafe { DestroySession(metadata.converter_handle as *mut c_void) };
        Ok(Response::new(CloseSessionResponse {}))
    }

    async fn append_text(
        &self,
        request: Request<AppendTextRequest>,
    ) -> Result<Response<AppendTextResponse>, Status> {
        let request = request.into_inner();
        let _guard = self.converter_lock.lock().await;
        let input = request.text_to_append;
        let revision = self.snapshot_manager.current();
        let composing_text = self
            .registry
            .with_mut(&request.session_id, move |session| {
                if !input.is_empty() && session.generation_pin.pinned_generation().is_none() {
                    if let Some(applied) = session.generation_pin.begin_composition(revision) {
                        apply_revision(
                            session.converter_handle as *mut c_void,
                            session.grimodex_integration_allowed(),
                            &applied,
                        )?;
                    }
                }
                let handle = session.converter_handle as *mut c_void;
                let composing_text = add_text(handle, &input);
                Ok(ComposingText {
                    hiragana: composing_text.text,
                    suggestions: get_composed_text(handle),
                })
            })
            .map_err(Status::failed_precondition)?;

        Ok(Response::new(AppendTextResponse {
            composing_text: Some(composing_text),
        }))
    }

    async fn remove_text(
        &self,
        request: Request<RemoveTextRequest>,
    ) -> Result<Response<RemoveTextResponse>, Status> {
        let session = self.require_session(&request.into_inner().session_id)?;
        let _guard = self.converter_lock.lock().await;
        let composing_text = remove_text(session.converter_handle as *mut c_void);

        Ok(Response::new(RemoveTextResponse {
            composing_text: Some(ComposingText {
                hiragana: composing_text.text,
                suggestions: get_composed_text(session.converter_handle as *mut c_void).to_vec(),
            }),
        }))
    }

    async fn move_cursor(
        &self,
        request: Request<MoveCursorRequest>,
    ) -> Result<Response<MoveCursorResponse>, Status> {
        let request = request.into_inner();
        let session = self.require_session(&request.session_id)?;
        let _guard = self.converter_lock.lock().await;
        let offset = request.offset as i8;
        let composing_text = move_cursor(session.converter_handle as *mut c_void, offset);

        Ok(Response::new(MoveCursorResponse {
            composing_text: Some(ComposingText {
                hiragana: composing_text.text,
                suggestions: get_composed_text(session.converter_handle as *mut c_void).to_vec(),
            }),
        }))
    }

    async fn clear_text(
        &self,
        request: Request<ClearTextRequest>,
    ) -> Result<Response<ClearTextResponse>, Status> {
        let session_id = request.into_inner().session_id;
        let _guard = self.converter_lock.lock().await;
        let revision = self.snapshot_manager.current();
        self.registry
            .with_mut(&session_id, |session| {
                if let Some(applied) = session.generation_pin.end_composition(revision) {
                    apply_revision(
                        session.converter_handle as *mut c_void,
                        session.grimodex_integration_allowed(),
                        &applied,
                    )?;
                }
                clear_text(session.converter_handle as *mut c_void);
                Ok(())
            })
            .map_err(Status::failed_precondition)?;
        Ok(Response::new(ClearTextResponse {}))
    }

    async fn shrink_text(
        &self,
        request: Request<ShrinkTextRequest>,
    ) -> Result<Response<ShrinkTextResponse>, Status> {
        let request = request.into_inner();
        let session = self.require_session(&request.session_id)?;
        let _guard = self.converter_lock.lock().await;
        let offset = request.offset as i8;
        let composing_text = shrink_text(session.converter_handle as *mut c_void, offset);

        Ok(Response::new(ShrinkTextResponse {
            composing_text: Some(ComposingText {
                hiragana: composing_text.text,
                suggestions: get_composed_text(session.converter_handle as *mut c_void).to_vec(),
            }),
        }))
    }

    async fn set_context(
        &self,
        request: Request<shared::proto::SetContextRequest>,
    ) -> Result<Response<shared::proto::SetContextResponse>, Status> {
        let request = request.into_inner();
        let session = self.require_session(&request.session_id)?;
        let _guard = self.converter_lock.lock().await;
        let context = request.context;
        let trimmed_context = context
            .split('\r')
            .filter(|s| !s.is_empty())
            .last()
            .unwrap_or_default();

        let context = CString::new(if session.secure { "" } else { trimmed_context })
            .map_err(|_| Status::invalid_argument("context contains NUL"))?;

        unsafe { SetContext(session.converter_handle as *mut c_void, context.as_ptr()) };
        Ok(Response::new(shared::proto::SetContextResponse {}))
    }

    async fn update_config(
        &self,
        request: Request<shared::proto::UpdateConfigRequest>,
    ) -> Result<Response<shared::proto::UpdateConfigResponse>, Status> {
        let request = request.into_inner();
        let session = self.require_session(&request.session_id)?;
        let _guard = self.converter_lock.lock().await;
        unsafe { LoadConfig(session.converter_handle as *mut c_void) };
        Ok(Response::new(shared::proto::UpdateConfigResponse {}))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("AzookeyServer started");
    // get executable directory
    let current_exe = std::env::current_exe()?;
    let parent_dir = current_exe
        .parent()
        .ok_or("server executable has no parent directory")?
        .to_path_buf();

    let snapshot_root = resolve_root();
    let registrar = ConsumerRegistrar::new(snapshot_root.clone(), "0.1.0");
    registrar.register()?;
    let heartbeat_registrar = registrar.clone();
    let _heartbeat = std::thread::Builder::new()
        .name("grimodex-consumer-heartbeat".to_string())
        .spawn(move || loop {
            std::thread::sleep(CONSUMER_HEARTBEAT);
            if let Err(error) = heartbeat_registrar.heartbeat() {
                eprintln!("Grimodex consumer heartbeat failed: {error}");
            }
        })?;

    let service = SessionAzookeyService::new(parent_dir, snapshot_root.clone());
    let watcher_service = service.clone();
    let user_data_paths = vec![snapshot_root.clone(), snapshot_root.join("projects")];
    watcher::spawn(
        user_data_paths,
        Arc::new(move || {
            watcher_service.reload_snapshot_and_sessions();
        }),
    );

    println!("AzookeyServer listening");

    let grpc_service = AzookeyServiceServer::new(service)
        .max_decoding_message_size(64 * 1024)
        .max_encoding_message_size(256 * 1024);

    Server::builder()
        .add_service(grpc_service)
        .add_service(
            ReflectionBuilder::configure()
                .register_encoded_file_descriptor_set(shared::proto::FILE_DESCRIPTOR_SET)
                .build_v1()?,
        )
        .serve_with_incoming(TonicNamedPipeServer::new(&server_pipe_name()))
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::SessionRegistry;

    #[test]
    fn sessions_are_scoped_and_must_be_opened() {
        let registry = SessionRegistry::default();
        assert!(registry.require("tsf-1").is_err());
        registry
            .open(
                "tsf-1".into(),
                SessionMetadata {
                    input_scope: "text".into(),
                    secure: false,
                    converter_handle: 0,
                    integration_allowed: false,
                    generation_pin: CompositionGenerationPin::default(),
                },
            )
            .unwrap();
        assert!(registry.require("tsf-1").is_ok());
        assert!(registry
            .open(
                "tsf-1".into(),
                SessionMetadata {
                    input_scope: "text".into(),
                    secure: false,
                    converter_handle: 0,
                    integration_allowed: false,
                    generation_pin: CompositionGenerationPin::default(),
                },
            )
            .is_err());
        registry.close("tsf-1").unwrap();
        assert!(registry.require("tsf-1").is_err());
    }

    #[test]
    fn session_ids_are_bounded_and_ascii() {
        assert!(SessionRegistry::validate_id("tsf-abc_01").is_ok());
        assert!(SessionRegistry::validate_id("bad id").is_err());
        assert!(SessionRegistry::validate_id(&"x".repeat(129)).is_err());
    }
}
