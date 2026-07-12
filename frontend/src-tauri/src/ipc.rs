use anyhow::Result;
use hyper_util::rt::TokioIo;
use shared::proto::{
    azookey_service_client::AzookeyServiceClient, OpenSessionRequest, UpdateConfigRequest,
};
use shared::server_pipe_name;
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{net::windows::named_pipe::ClientOptions, time};
use tonic::transport::Endpoint;
use tower::service_fn;
use windows::Win32::Foundation::ERROR_PIPE_BUSY;

// connect to kkc server
#[derive(Debug, Clone)]
pub struct IPCService {
    // kkc server client
    azookey_client: AzookeyServiceClient<tonic::transport::channel::Channel>,
    runtime: Arc<tokio::runtime::Runtime>,
    session_id: String,
}

impl IPCService {
    pub fn new() -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new()?;
        let session_id = format!(
            "ui-control-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or_default()
        );

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

        let mut azookey_client = AzookeyServiceClient::new(server_channel);
        runtime.block_on(
            azookey_client.open_session(tonic::Request::new(OpenSessionRequest {
                session_id: session_id.clone(),
                input_scope: "control".to_string(),
                secure: true,
                application_id: "control".to_string(),
            })),
        )?;

        Ok(Self {
            azookey_client,
            runtime: Arc::new(runtime),
            session_id,
        })
    }
}

// implement methods to interact with kkc server
impl IPCService {
    pub fn update_config(&mut self) -> anyhow::Result<()> {
        let request = tonic::Request::new(UpdateConfigRequest {
            session_id: self.session_id.clone(),
        });
        self.runtime
            .clone()
            .block_on(self.azookey_client.update_config(request))?;

        Ok(())
    }
}
