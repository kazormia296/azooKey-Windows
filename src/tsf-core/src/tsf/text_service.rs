use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use windows::{
    core::{implement, AsImpl, Interface, GUID},
    Win32::UI::TextServices::{
        ITfCompartmentEventSink, ITfCompositionSink, ITfContext, ITfDisplayAttributeProvider,
        ITfKeyEventSink, ITfLangBarItem, ITfLangBarItemButton, ITfLangBarItemSink, ITfSource,
        ITfTextInputProcessor, ITfTextInputProcessorEx, ITfTextLayoutSink, ITfThreadMgr,
        ITfThreadMgrEventSink,
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
    pub display_attribute_atom: Cell<HashMap<GUID, u32>>,
    pub input_mode: Cell<InputMode>,
    pub lang_bar_item_sink: Cell<Option<ITfLangBarItemSink>>,
    pub contexts: RefCell<ContextManager>,
    pub compartments: RefCell<HashMap<GUID, CompartmentEntry>>,
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

    pub fn get_active_context(&self) -> Result<ITfContext> {
        let thread_mgr = self.thread_mgr()?;
        unsafe {
            let doc_mgr = thread_mgr.GetFocus()?;
            let context = doc_mgr.GetTop()?;
            Ok(context)
        }
    }
}
