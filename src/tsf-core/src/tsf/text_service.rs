use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use windows::{
    core::{implement, AsImpl, Interface, GUID},
    Win32::UI::TextServices::{
        ITfCompartmentEventSink, ITfCompositionSink, ITfDisplayAttributeProvider,
        ITfKeyEventSink, ITfLangBarItem, ITfLangBarItemButton, ITfLangBarItemSink,
        ITfSource, ITfTextInputProcessor, ITfTextInputProcessorEx, ITfTextLayoutSink,
        ITfThreadMgr, ITfThreadMgrEventSink,
    },
};

use anyhow::{Context, Result};

use crate::{engine::input_mode::InputMode, tsf::compartment::CompartmentEntry};

use super::context::ContextManager;

#[derive(Default)]
#[implement(
    ITfTextInputProcessor,
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfThreadMgrEventSink,
    ITfTextLayoutSink,
    ITfCompositionSink,
    ITfDisplayAttributeProvider,
    ITfLangBarItem,
    ITfLangBarItemButton,
    ITfSource,
    ITfCompartmentEventSink
)]
pub struct TextService {
    this: Cell<Option<ITfTextInputProcessor>>,
    pub tid: Cell<u32>,
    pub thread_mgr: Cell<Option<ITfThreadMgr>>,
    pub thread_mgr_event_sink_cookie: Cell<Option<u32>>,
    pub display_attribute_atom: RefCell<HashMap<GUID, u32>>,
    pub input_mode: Cell<InputMode>,
    pub lang_bar_item_sink: Cell<Option<ITfLangBarItemSink>>,
    pub contexts: RefCell<ContextManager>,
    pub compartments: RefCell<HashMap<GUID, CompartmentEntry>>,
}

impl std::fmt::Debug for TextService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("TextService");
        s.field("tid", &self.tid.get());
        s.field("input_mode", &self.input_mode.get());
        s.field("thread_mgr_event_sink_cookie", &self.thread_mgr_event_sink_cookie.get());
        let this = self.this.take();
        s.field("this", &this);
        self.this.set(this);
        let thread_mgr = self.thread_mgr.take();
        s.field("thread_mgr", &thread_mgr);
        self.thread_mgr.set(thread_mgr);
        let lang_bar_item_sink = self.lang_bar_item_sink.take();
        s.field("lang_bar_item_sink", &lang_bar_item_sink);
        self.lang_bar_item_sink.set(lang_bar_item_sink);
        let keys: Vec<GUID> = self.compartments.borrow().keys().copied().collect();
        s.field("compartments", &keys);
        s.finish_non_exhaustive()
    }
}

impl TextService {
    pub fn create<I: Interface>() -> Result<I> {
        let factory = Self::default();
        let this = ITfTextInputProcessor::from(factory);
        let factory = unsafe { this.as_impl() };
        factory.this.set(Some(this.clone()));
        unsafe { factory.cast::<I>().map_err(|e| anyhow::Error::new(e)) }
    }

    pub fn this<I: Interface>(&self) -> Result<I> {
        let this = self.this.take().context("this is null")?;
        let result = this.clone().cast()?;
        self.this.set(Some(this));
        Ok(result)
    }

    pub fn thread_mgr(&self) -> Result<ITfThreadMgr> {
        let thread_mgr = self.thread_mgr.take().context("Thread manager is null")?;
        let result = thread_mgr.clone();
        self.thread_mgr.set(Some(thread_mgr));
        Ok(result)
    }
}
