use std::cell::Cell;

use std::collections::HashMap;

use anyhow::Result;
use windows::{
    core::Interface,
    Win32::UI::TextServices::{ITfComposition, ITfContext, ITfSource},
};

pub struct ContextState {
    pub context: ITfContext,
    pub composition: Cell<Option<ITfComposition>>,
    pub text_edit_sink_cookie: Cell<Option<u32>>,
    pub text_layout_sink_cookie: Cell<Option<u32>>,
}

impl ContextState {
    pub fn set_composition(&self, comp: Option<ITfComposition>) {
        self.composition.set(comp);
    }

    pub fn take_composition(&self) -> Option<ITfComposition> {
        self.composition.take()
    }

    pub fn unadvise_text_layout_sink(&self) -> Result<()> {
        if let Some(cookie) = self.text_layout_sink_cookie.take() {
            unsafe {
                self.context.cast::<ITfSource>()?.UnadviseSink(cookie)?;
            }
        }
        Ok(())
    }

    pub fn unadvise_text_edit_sink(&self) -> Result<()> {
        if let Some(cookie) = self.text_edit_sink_cookie.take() {
            unsafe {
                self.context.cast::<ITfSource>()?.UnadviseSink(cookie)?;
            }
        }
        Ok(())
    }

    pub fn unadvise_all(&self) -> Result<()> {
        self.unadvise_text_layout_sink()?;
        self.unadvise_text_edit_sink()?;
        Ok(())
    }
}

impl Drop for ContextState {
    fn drop(&mut self) {
        let _ = self.unadvise_all();
    }
}

#[derive(Default)]
pub struct ContextManager {
    registry: HashMap<isize, ContextState>,
}

impl ContextManager {
    fn key(context: &ITfContext) -> isize {
        context.as_raw() as isize
    }

    pub fn register(&mut self, context: &ITfContext) {
        let key = Self::key(context);
        self.registry.insert(
            key,
            ContextState {
                context: context.clone(),
                composition: Cell::new(None),
                text_edit_sink_cookie: Cell::new(None),
                text_layout_sink_cookie: Cell::new(None),
            },
        );
    }

    pub fn unregister(&mut self, context: &ITfContext) -> Result<()> {
        if let Some(state) = self.registry.remove(&Self::key(context)) {
            state.unadvise_all()?;
        }
        Ok(())
    }

    pub fn find(&self, context: &ITfContext) -> Option<&ContextState> {
        self.registry.get(&Self::key(context))
    }

    pub fn set_text_layout_cookie(&self, context: &ITfContext, cookie: u32) {
        if let Some(state) = self.registry.get(&Self::key(context)) {
            state.text_layout_sink_cookie.set(Some(cookie));
        }
    }

    pub fn clear(&mut self) {
        self.registry.clear();
    }
}
