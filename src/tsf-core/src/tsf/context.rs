use std::collections::VecDeque;

use anyhow::Result;
use windows::{
    core::Interface,
    Win32::UI::TextServices::{ITfComposition, ITfContext, ITfSource},
};

#[derive(Debug)]
pub struct ContextState {
    pub context: ITfContext,
    pub composition: Option<ITfComposition>,
    pub text_edit_sink_cookie: Option<u32>,
    pub text_layout_sink_cookie: Option<u32>,
}

impl ContextState {
    /// text_layout_sink を解除する
    pub fn unadvise_text_layout_sink(&mut self) -> Result<()> {
        if let Some(cookie) = self.text_layout_sink_cookie.take() {
            unsafe {
                self.context.cast::<ITfSource>()?.UnadviseSink(cookie)?;
            }
        }
        Ok(())
    }

    /// text_edit_sink を解除する
    pub fn unadvise_text_edit_sink(&mut self) -> Result<()> {
        if let Some(cookie) = self.text_edit_sink_cookie.take() {
            unsafe {
                self.context.cast::<ITfSource>()?.UnadviseSink(cookie)?;
            }
        }
        Ok(())
    }
}

impl Drop for ContextState {
    fn drop(&mut self) {
        let _ = self.unadvise_text_layout_sink();
        let _ = self.unadvise_text_edit_sink();
    }
}

#[derive(Debug, Default)]
pub struct ContextManager {
    stack: VecDeque<ContextState>,
}

impl ContextManager {
    pub fn active(&self) -> Option<&ContextState> {
        self.stack.back()
    }

    pub fn active_mut(&mut self) -> Option<&mut ContextState> {
        self.stack.back_mut()
    }

    /// ITfContext のアドレスでコンテキストを検索する
    pub fn find_mut(&mut self, context: &ITfContext) -> Option<&mut ContextState> {
        self.stack
            .iter_mut()
            .find(|state| state.context.as_raw() == context.as_raw())
    }

    /// スタック内の全コンテキストを可変で参照する
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ContextState> {
        self.stack.iter_mut()
    }

    pub fn push(&mut self, context: ITfContext) {
        self.stack.push_back(ContextState {
            context,
            composition: None,
            text_edit_sink_cookie: None,
            text_layout_sink_cookie: None,
        });
    }

    pub fn pop(&mut self, context: &ITfContext) -> Option<ContextState> {
        let pos = self
            .stack
            .iter()
            .position(|state| state.context.as_raw() == context.as_raw())?;
        self.stack.remove(pos)
    }

    /// スタックを空にし、全 ContextState を返す
    pub fn drain(&mut self) -> Vec<ContextState> {
        self.stack.drain(..).collect()
    }
}
