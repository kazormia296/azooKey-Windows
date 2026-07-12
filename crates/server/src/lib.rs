use async_stream::stream;
use futures_core::stream::Stream;
use std::{ffi::c_void, os::windows::io::AsRawHandle, pin::Pin, ptr::addr_of_mut};
use tokio::{
    io::{self, AsyncRead, AsyncWrite},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
};
use tonic::transport::server::Connected;
use windows::{
    core::{w, PWSTR},
    Win32::Foundation::{CloseHandle, HANDLE},
    Win32::Security::{
        Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION},
        PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
    },
    Win32::System::{
        Pipes::GetNamedPipeClientProcessId,
        Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
};

const MAX_PIPE_INSTANCES: u32 = 64;

#[allow(dead_code)]
struct UnsafeSecurityAttributes(SECURITY_ATTRIBUTES);

unsafe impl Send for UnsafeSecurityAttributes {}
unsafe impl Sync for UnsafeSecurityAttributes {}

pub struct TonicNamedPipeServer {
    inner: NamedPipeServer,
}

impl Connected for TonicNamedPipeServer {
    type ConnectInfo = ();

    fn connect_info(&self) -> Self::ConnectInfo {
        ()
    }
}

impl AsyncRead for TonicNamedPipeServer {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for TonicNamedPipeServer {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl TonicNamedPipeServer {
    fn create_security_descriptor() -> io::Result<usize> {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                w!("D:(A;;GA;;;OW)(A;;GA;;;SY)(A;;GA;;;AC)(A;;GA;;;RC)S:(ML;;NW;;;LW)"),
                SDDL_REVISION,
                &mut descriptor,
                None,
            )
            .map_err(|error| io::Error::other(format!("pipe security descriptor: {error}")))?;
        }
        Ok(descriptor.0 as usize)
    }

    fn verify_client_process(server: &NamedPipeServer) -> io::Result<u32> {
        let mut pid = 0u32;
        unsafe {
            GetNamedPipeClientProcessId(HANDLE(server.as_raw_handle()), &mut pid)
                .map_err(|error| io::Error::other(format!("named pipe client PID: {error}")))?;
            if pid == 0 {
                return Err(io::Error::other("named pipe client PID is zero"));
            }

            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
                .map_err(|error| io::Error::other(format!("open named pipe client: {error}")))?;
            let mut path = [0u16; 32_768];
            let mut length = path.len() as u32;
            let result = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(path.as_mut_ptr()),
                &mut length,
            )
            .map_err(|error| io::Error::other(format!("query named pipe client image: {error}")));
            let _ = CloseHandle(process);
            result?;
            if length == 0 {
                return Err(io::Error::other("named pipe client image is empty"));
            }
        }
        Ok(pid)
    }

    pub fn new(path: &str) -> impl Stream<Item = io::Result<TonicNamedPipeServer>> {
        // set security attributes to allow ipc from sandboxed processes
        // see https://nathancorvussolis.blogspot.com/2018/05/windows-ime-security.html

        let name = if path.starts_with(r"\\.\pipe\") {
            path.to_string()
        } else {
            format!("\\\\.\\pipe\\{}", path)
        };

        stream! {
            let descriptor_address = match Self::create_security_descriptor() {
                Ok(address) => address,
                Err(error) => {
                    yield Err(error);
                    return;
                }
            };

            let mut security_attributes = UnsafeSecurityAttributes(SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor_address as *mut c_void,
                bInheritHandle: false.into(),
            });

            let mut server = unsafe {
                ServerOptions::new()
                    .first_pipe_instance(true)
                    .max_instances(MAX_PIPE_INSTANCES as usize)
                    .create_with_security_attributes_raw(
                        &name,
                        addr_of_mut!(security_attributes) as *mut c_void
                    )
            }?;

            loop {
                server.connect().await?;

                // The owner/SYSTEM/AppContainer ACL prevents cross-user
                // access; verify the client PID and image before handing the
                // connection to tonic so spoofed transports fail closed.
                if let Err(error) = Self::verify_client_process(&server) {
                    yield Err(error);
                    server = unsafe {
                        ServerOptions::new()
                            .max_instances(MAX_PIPE_INSTANCES as usize)
                            .create_with_security_attributes_raw(
                                &name,
                                addr_of_mut!(security_attributes) as *mut c_void,
                            )
                    }?;
                    continue;
                }

                let client = TonicNamedPipeServer { inner: server };

                yield Ok(client);

                server = unsafe {
                    ServerOptions::new()
                        .max_instances(MAX_PIPE_INSTANCES as usize)
                        .create_with_security_attributes_raw(
                            &name,
                            addr_of_mut!(security_attributes) as *mut c_void,
                        )
                }?;
            }
        }
    }
}
