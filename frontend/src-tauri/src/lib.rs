mod ipc;

use serde::{Deserialize, Serialize};
use shared::AppConfig;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

const ZENZAI_MODEL_URL: &str = "https://github.com/kazormia296/grimodex-models/releases/download/zenzai-v3-small-q5km-v1/zenzai-v3-small-Q5_K_M.gguf";
const ZENZAI_MODEL_SHA256: &str =
    "501f605d088f5b988791a00ae19ed46985ed7c48144f364b2f3f1f951c9b2083";

fn zenzai_model_path() -> Result<PathBuf, String> {
    Ok(shared::get_config_root().join("zenz.gguf"))
}

fn has_zenzai_model(path: &Path) -> bool {
    path.is_file()
}

#[derive(Debug)]
pub struct AppState {
    settings: Mutex<AppConfig>,
    ipc: ipc::IPCService,
}

impl AppState {
    fn new() -> Self {
        AppState {
            settings: Mutex::new(AppConfig::new()),
            ipc: ipc::IPCService::new().unwrap(),
        }
    }
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_config(state: tauri::State<AppState>) -> AppConfig {
    let config = state.settings.lock().unwrap();
    config.clone()
}

#[tauri::command]
fn update_config(state: tauri::State<AppState>, new_config: AppConfig) {
    let mut config = state.settings.lock().unwrap();
    *config = new_config;
    config.write();

    state.ipc.clone().update_config().unwrap();
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Capability {
    cpu: bool,
    cuda: bool,
    vulkan: bool,
}

#[tauri::command]
fn check_capability() -> Capability {
    // cuda:
    // cudart64_12.dll
    // cublas64_12.dll

    // vulkan:
    // vulkan-1.dllの存在確認

    let mut capability = Capability {
        cpu: true,
        cuda: false,
        vulkan: false,
    };

    // Check for CUDA availability
    let cuda_files = ["cudart64_12.dll", "cublas64_12.dll"];
    let cuda_available = cuda_files.iter().all(|file| {
        // Check if the file exists in system path or in the current directory
        std::env::var("PATH")
            .unwrap_or_default()
            .split(';')
            .map(PathBuf::from)
            .chain(std::iter::once(std::env::current_dir().unwrap_or_default()))
            .any(|path| path.join(file).exists())
    });
    capability.cuda = cuda_available;

    // Check for Vulkan availability
    let vulkan_file = "vulkan-1.dll";
    let vulkan_available = std::env::var("PATH")
        .unwrap_or_default()
        .split(';')
        .map(PathBuf::from)
        .chain(std::iter::once(std::env::current_dir().unwrap_or_default()))
        .any(|path| path.join(vulkan_file).exists());
    capability.vulkan = vulkan_available;

    capability
}

#[tauri::command]
fn zenzai_model_status() -> Result<bool, String> {
    Ok(has_zenzai_model(&zenzai_model_path()?))
}

#[tauri::command]
fn download_zenzai_model(state: tauri::State<AppState>) -> Result<(), String> {
    let output = zenzai_model_path()?;
    let temporary = output.with_extension("gguf.download");
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Zenzaiモデルの保存先を作成できませんでした: {error}"))?;
    }
    let script = r#"
$ErrorActionPreference = 'Stop'
$output = $env:GRIMODEX_ZENZAI_OUTPUT
$temporary = "$output.download"
try {
    Invoke-WebRequest -Uri $env:GRIMODEX_ZENZAI_URL -OutFile $temporary -MaximumRedirection 5
    $hash = (Get-FileHash -Algorithm SHA256 $temporary).Hash.ToLowerInvariant()
    if ($hash -ne $env:GRIMODEX_ZENZAI_SHA256) {
        throw "Zenzai model checksum verification failed"
    }
    Move-Item -Force $temporary $output
} finally {
    if (Test-Path $temporary) {
        Remove-Item -Force $temporary
    }
}
"#;

    let status = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
            script,
        ])
        .env("GRIMODEX_ZENZAI_OUTPUT", &output)
        .env("GRIMODEX_ZENZAI_URL", ZENZAI_MODEL_URL)
        .env("GRIMODEX_ZENZAI_SHA256", ZENZAI_MODEL_SHA256)
        .status()
        .map_err(|error| format!("Zenzaiモデルのダウンローダーを起動できませんでした: {error}"))?;
    if !status.success() {
        let _ = fs::remove_file(&temporary);
        return Err("Zenzaiモデルのダウンロードまたは検証に失敗しました".to_string());
    }

    state
        .ipc
        .clone()
        .update_config()
        .map_err(|error| format!("Zenzaiモデルの反映に失敗しました: {error}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState::new();

    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_config,
            update_config,
            check_capability,
            zenzai_model_status,
            download_zenzai_model
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
