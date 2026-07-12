use anyhow::Result;
use hyper_util::rt::TokioIo;
use shared::{
    proto::{
        azookey_service_client::AzookeyServiceClient, window_service_client::WindowServiceClient,
        CloseSessionRequest, OpenSessionRequest,
    },
    server_pipe_name, ui_pipe_name,
};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{net::windows::named_pipe::ClientOptions, time};
use tonic::transport::Endpoint;
use tower::service_fn;
use windows::Win32::Foundation::ERROR_PIPE_BUSY;

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

fn session_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!(
        "tsf-{}-{}-{}",
        std::process::id(),
        timestamp,
        NEXT_SESSION.fetch_add(1, Ordering::Relaxed)
    )
}

// connect to kkc server
#[derive(Debug, Clone)]
pub struct IPCService {
    // kkc server client
    azookey_client: AzookeyServiceClient<tonic::transport::channel::Channel>,
    // candidate window server client
    window_client: WindowServiceClient<tonic::transport::channel::Channel>,
    runtime: Arc<tokio::runtime::Runtime>,
    session_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct Candidates {
    pub texts: Vec<String>,
    pub sub_texts: Vec<String>,
    pub hiragana: String,
    pub corresponding_count: Vec<i32>,
}

impl IPCService {
    pub fn new() -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new()?;
        let session_id = session_id();

        let server_channel = runtime.block_on(
            Endpoint::try_from("http://[::]:50051")?.connect_with_connector(service_fn(
                |_| async {
                    let client = time::timeout(Duration::from_secs(10), async {
                        loop {
                            match ClientOptions::new().open(server_pipe_name()) {
                                Ok(client) => break Ok(client),
                                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) => (),
                                Err(e) => break Err(e),
                            }
                            time::sleep(Duration::from_millis(50)).await;
                        }
                    })
                    .await
                    .map_err(|_| {
                        std::io::Error::new(std::io::ErrorKind::TimedOut, "server pipe timeout")
                    })??;

                    Ok::<_, std::io::Error>(TokioIo::new(client))
                },
            )),
        )?;

        let ui_channel = runtime.block_on(
            Endpoint::try_from("http://[::]:50052")?.connect_with_connector(service_fn(
                |_| async {
                    let client = time::timeout(Duration::from_secs(10), async {
                        loop {
                            match ClientOptions::new().open(ui_pipe_name()) {
                                Ok(client) => break Ok(client),
                                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) => (),
                                Err(e) => break Err(e),
                            }
                            time::sleep(Duration::from_millis(50)).await;
                        }
                    })
                    .await
                    .map_err(|_| {
                        std::io::Error::new(std::io::ErrorKind::TimedOut, "ui pipe timeout")
                    })??;

                    Ok::<_, std::io::Error>(TokioIo::new(client))
                },
            )),
        )?;

        let mut azookey_client = AzookeyServiceClient::new(server_channel);
        let window_client = WindowServiceClient::new(ui_channel);
        runtime.block_on(
            azookey_client.open_session(tonic::Request::new(OpenSessionRequest {
                session_id: session_id.clone(),
                input_scope: "text".to_string(),
                secure: false,
            })),
        )?;
        tracing::debug!("Connected to server: {:?}", azookey_client);

        Ok(Self {
            azookey_client,
            window_client,
            runtime: Arc::new(runtime),
            session_id,
        })
    }
}

// implement methods to interact with kkc server
impl IPCService {
    #[tracing::instrument]
    pub fn append_text(&mut self, text: String) -> anyhow::Result<Candidates> {
        let request = tonic::Request::new(shared::proto::AppendTextRequest {
            text_to_append: text,
            session_id: self.session_id.clone(),
        });

        let response = self
            .runtime
            .clone()
            .block_on(self.azookey_client.append_text(request))?;
        let composing_text = response.into_inner().composing_text;

        let candidates = if let Some(composing_text) = composing_text {
            Candidates {
                texts: composing_text
                    .suggestions
                    .iter()
                    .map(|s| s.text.clone())
                    .collect(),
                sub_texts: composing_text
                    .suggestions
                    .iter()
                    .map(|s| s.subtext.clone())
                    .collect(),
                hiragana: composing_text.hiragana,
                corresponding_count: composing_text
                    .suggestions
                    .iter()
                    .map(|s| s.corresponding_count)
                    .collect(),
            }
        } else {
            anyhow::bail!("composing_text is None");
        };

        Ok(candidates)
    }

    #[tracing::instrument]
    pub fn remove_text(&mut self) -> anyhow::Result<Candidates> {
        let request = tonic::Request::new(shared::proto::RemoveTextRequest {
            session_id: self.session_id.clone(),
        });
        let response = self
            .runtime
            .clone()
            .block_on(self.azookey_client.remove_text(request))?;
        let composing_text = response.into_inner().composing_text;

        let candidates = if let Some(composing_text) = composing_text {
            Candidates {
                texts: composing_text
                    .suggestions
                    .iter()
                    .map(|s| s.text.clone())
                    .collect(),
                sub_texts: composing_text
                    .suggestions
                    .iter()
                    .map(|s| s.subtext.clone())
                    .collect(),
                hiragana: composing_text.hiragana,
                corresponding_count: composing_text
                    .suggestions
                    .iter()
                    .map(|s| s.corresponding_count)
                    .collect(),
            }
        } else {
            anyhow::bail!("composing_text is None");
        };

        Ok(candidates)
    }

    #[tracing::instrument]
    pub fn clear_text(&mut self) -> anyhow::Result<()> {
        let request = tonic::Request::new(shared::proto::ClearTextRequest {
            session_id: self.session_id.clone(),
        });
        let _response = self
            .runtime
            .clone()
            .block_on(self.azookey_client.clear_text(request))?;

        Ok(())
    }

    #[tracing::instrument]
    pub fn shrink_text(&mut self, offset: i32) -> anyhow::Result<Candidates> {
        let request = tonic::Request::new(shared::proto::ShrinkTextRequest {
            offset,
            session_id: self.session_id.clone(),
        });
        let response = self
            .runtime
            .clone()
            .block_on(self.azookey_client.shrink_text(request))?;
        let composing_text = response.into_inner().composing_text;

        let candidates = if let Some(composing_text) = composing_text {
            Candidates {
                texts: composing_text
                    .suggestions
                    .iter()
                    .map(|s| s.text.clone())
                    .collect(),
                sub_texts: composing_text
                    .suggestions
                    .iter()
                    .map(|s| s.subtext.clone())
                    .collect(),
                hiragana: composing_text.hiragana,
                corresponding_count: composing_text
                    .suggestions
                    .iter()
                    .map(|s| s.corresponding_count)
                    .collect(),
            }
        } else {
            anyhow::bail!("composing_text is None");
        };

        Ok(candidates)
    }

    pub fn set_context(&mut self, context: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(shared::proto::SetContextRequest {
            context,
            session_id: self.session_id.clone(),
        });
        let _response = self
            .runtime
            .clone()
            .block_on(self.azookey_client.set_context(request))?;

        Ok(())
    }

    pub fn close_session(&mut self) -> anyhow::Result<()> {
        self.runtime
            .clone()
            .block_on(self.azookey_client.close_session(tonic::Request::new(
                CloseSessionRequest {
                    session_id: self.session_id.clone(),
                },
            )))?;
        Ok(())
    }
}

// implement methods to interact with candidate window server
impl IPCService {
    #[tracing::instrument]
    pub fn show_window(&mut self) -> anyhow::Result<()> {
        let request = tonic::Request::new(shared::proto::EmptyResponse {});
        self.runtime
            .clone()
            .block_on(self.window_client.show_window(request))?;

        Ok(())
    }

    #[tracing::instrument]
    pub fn hide_window(&mut self) -> anyhow::Result<()> {
        let request = tonic::Request::new(shared::proto::EmptyResponse {});
        self.runtime
            .clone()
            .block_on(self.window_client.hide_window(request))?;

        Ok(())
    }

    #[tracing::instrument]
    pub fn set_window_position(
        &mut self,
        top: i32,
        left: i32,
        bottom: i32,
        right: i32,
    ) -> anyhow::Result<()> {
        let request = tonic::Request::new(shared::proto::SetPositionRequest {
            position: Some(shared::proto::WindowPosition {
                top,
                left,
                bottom,
                right,
            }),
        });
        self.runtime
            .clone()
            .block_on(self.window_client.set_window_position(request))?;

        Ok(())
    }

    #[tracing::instrument]
    pub fn set_candidates(&mut self, candidates: Vec<String>) -> anyhow::Result<()> {
        let request = tonic::Request::new(shared::proto::SetCandidateRequest { candidates });
        self.runtime
            .clone()
            .block_on(self.window_client.set_candidate(request))?;

        Ok(())
    }

    #[tracing::instrument]
    pub fn set_selection(&mut self, index: i32) -> anyhow::Result<()> {
        let request = tonic::Request::new(shared::proto::SetSelectionRequest { index });
        self.runtime
            .clone()
            .block_on(self.window_client.set_selection(request))?;

        Ok(())
    }

    #[tracing::instrument]
    pub fn set_input_mode(&mut self, mode: &str) -> anyhow::Result<()> {
        let request = tonic::Request::new(shared::proto::SetInputModeRequest {
            mode: mode.to_string(),
        });
        self.runtime
            .clone()
            .block_on(self.window_client.set_input_mode(request))?;

        Ok(())
    }
}
