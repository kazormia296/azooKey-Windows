use std::collections::HashMap;

use windows::{
    core::{Interface, GUID},
    Win32::UI::TextServices::{ITfContext, ITfTextInputProcessor, ITfThreadMgr},
};

use anyhow::{Context, Result};

use crate::engine::input_mode::InputMode;

#[derive(Default, Debug)]
pub struct TextServiceInner {
    pub tid: u32,
    pub thread_mgr: Option<ITfThreadMgr>,
    pub context: Option<ITfContext>,
    pub display_attribute_atom: HashMap<GUID, u32>,
    pub mode: InputMode,
    pub this: Option<ITfTextInputProcessor>,
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

    pub fn context<I: Interface>(&self) -> Result<I> {
        let context = self.context.as_ref().context("Context is null")?;
        Ok(context.cast()?)
    }
}
