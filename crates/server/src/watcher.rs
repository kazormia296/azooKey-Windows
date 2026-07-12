//! Product-scoped user-data watcher.
//!
//! Windows uses the native `ReadDirectoryChangesW` API so dictionary/config
//! updates do not require polling and handles are closed as soon as a watch
//! iteration finishes.  The callback is debounced because editors commonly
//! emit a write/rename pair for one save.

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use windows::{
        core::HSTRING,
        Win32::{
            Foundation::{CloseHandle, HANDLE},
            Storage::FileSystem::{
                CreateFileW, ReadDirectoryChangesW, FILE_FLAG_BACKUP_SEMANTICS,
                FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE, FILE_NOTIFY_CHANGE_CREATION,
                FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE,
                FILE_NOTIFY_CHANGE_SIZE, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
                OPEN_EXISTING,
            },
        },
    };

    pub fn spawn(paths: Vec<PathBuf>, callback: Arc<dyn Fn() + Send + Sync + 'static>) {
        let _ = std::thread::Builder::new()
            .name("grimodex-user-data-watcher".to_string())
            .spawn(move || loop {
                for path in &paths {
                    if let Err(error) = watch_once(path, &callback) {
                        eprintln!("Grimodex user-data watcher: {error}");
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
            });
    }

    fn watch_once(
        path: &PathBuf,
        callback: &Arc<dyn Fn() + Send + Sync + 'static>,
    ) -> std::io::Result<()> {
        let directory = unsafe {
            CreateFileW(
                &HSTRING::from(path.to_string_lossy().as_ref()),
                FILE_LIST_DIRECTORY.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                HANDLE::default(),
            )
        }
        .map_err(|error| std::io::Error::other(format!("open {}: {error}", path.display())))?;

        let mut buffer = [0u8; 64 * 1024];
        let mut bytes_returned = 0u32;
        let result = unsafe {
            ReadDirectoryChangesW(
                directory,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                true,
                FILE_NOTIFY_CHANGE(
                    FILE_NOTIFY_CHANGE_FILE_NAME.0
                        | FILE_NOTIFY_CHANGE_LAST_WRITE.0
                        | FILE_NOTIFY_CHANGE_SIZE.0
                        | FILE_NOTIFY_CHANGE_CREATION.0,
                ),
                Some(&mut bytes_returned),
                None,
                None,
            )
        };
        let _ = unsafe { CloseHandle(directory) };
        result
            .map_err(|error| std::io::Error::other(format!("watch {}: {error}", path.display())))?;

        if bytes_returned > 0 {
            // Debounce a save burst while keeping the watch handle closed.
            let started = Instant::now();
            std::thread::sleep(Duration::from_millis(100));
            if started.elapsed() >= Duration::from_millis(100) {
                callback();
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
pub use windows_impl::spawn;

#[cfg(not(windows))]
pub fn spawn(_paths: Vec<PathBuf>, _callback: Arc<dyn Fn() + Send + Sync + 'static>) {}
