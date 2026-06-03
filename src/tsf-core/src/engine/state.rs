// TODO: InputModeはサーバー側で持つようにしてみないか...?
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard},
};

use windows::{core::GUID, Win32::UI::TextServices::ITfContext};

use super::input_mode::InputMode;

#[derive(Debug)]
pub struct IMEState {
    pub input_mode: InputMode,
    pub cookies: HashMap<GUID, u32>,
    // ThreadMgrEventSinkとかTextLayoutSinkの実装に使う
    pub context: Option<ITfContext>,
    // 以前のITfContextを保存して、2重にadvise_text_layout_sinkしないようにしているみたい。必要性を検証する必要がありそう。
    // ThreadMgrEventSinkにおいてはprev_contextが引数として取られているみたいだから必要性がよくわからない
}

pub static IME_STATE: LazyLock<Mutex<IMEState>> = LazyLock::new(|| {
    tracing::debug!("Creating IMEState");
    Mutex::new(IMEState {
        input_mode: InputMode::default(),
        cookies: HashMap::new(),
        context: None,
    })
});
unsafe impl Sync for IMEState {}
unsafe impl Send for IMEState {}

impl IMEState {
    pub fn get() -> anyhow::Result<MutexGuard<'static, IMEState>> {
        match IME_STATE.try_lock() {
            Ok(guard) => Ok(guard),
            Err(e) => anyhow::bail!("Failed to lock state: {:?}", e),
        }
    }
}
