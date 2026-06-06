use std::collections::HashMap;

use windows::{
    core::{Interface, GUID},
    Win32::UI::TextServices::{ITfLangBarItemSink, ITfTextInputProcessor, ITfThreadMgr},
};

use anyhow::{Context, Result};

use crate::engine::input_mode::InputMode;

use super::context::ContextManager;

#[derive(Default, Debug)]
pub struct TextServiceInner {
    pub tid: u32,
    pub thread_mgr: Option<ITfThreadMgr>,
    pub thread_mgr_event_sink_cookie: Option<u32>,
    pub display_attribute_atom: HashMap<GUID, u32>,
    pub input_mode: InputMode,
    pub this: Option<ITfTextInputProcessor>,
    pub lang_bar_item_sink: Option<ITfLangBarItemSink>,
    pub contexts: ContextManager,
}

impl TextServiceInner {
    pub fn this<I: Interface>(&self) -> Result<I> {
        if let Some(this) = self.this.as_ref() {
            Ok(this.cast()?)
        } else {
            anyhow::bail!("this is null");
        }
    }

    pub fn thread_mgr(&self) -> Result<ITfThreadMgr> {
        self.thread_mgr.clone().context("Thread manager is null")
    }
}
