use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::c_void,
};

use windows::{
    core::{implement, AsImpl, IUnknown, Interface, GUID},
    Win32::{
        Foundation::{BOOL, E_NOINTERFACE},
        System::Com::{IClassFactory, IClassFactory_Impl},
        UI::TextServices::{
            ITfCompositionSink, ITfDisplayAttributeProvider, ITfKeyEventSink, ITfLangBarItem,
            ITfLangBarItemButton, ITfLangBarItemSink, ITfSource, ITfTextInputProcessor,
            ITfTextInputProcessorEx, ITfTextLayoutSink, ITfThreadMgr, ITfThreadMgrEventSink,
        },
    },
};

use anyhow::{Context, Result};

use crate::{engine::input_mode::InputMode, globals::DllModule};

use super::context::ContextManager;

#[derive(Default)]
#[implement(
    IClassFactory,
    ITfTextInputProcessor,
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfThreadMgrEventSink,
    ITfTextLayoutSink,
    ITfCompositionSink,
    ITfDisplayAttributeProvider,
    ITfLangBarItem,
    ITfLangBarItemButton,
    ITfSource
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
        s.finish_non_exhaustive()
    }
}

impl IClassFactory_Impl for TextService_Impl {
    #[macros::anyhow]
    fn CreateInstance(
        &self,
        punkouter: Option<&IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> Result<()> {
        let riid = unsafe { *riid };
        let ppvobject = unsafe { &mut *ppvobject };

        *ppvobject = std::ptr::null_mut();

        if punkouter.is_some() {
            return Err(windows::core::Error::from_hresult(E_NOINTERFACE).into());
        }

        unsafe {
            *ppvobject = match riid {
                ITfTextInputProcessor::IID => {
                    std::mem::transmute::<ITfTextInputProcessor, *mut c_void>(
                        TextService::create::<ITfTextInputProcessor>()?,
                    )
                }
                ITfTextInputProcessorEx::IID => {
                    std::mem::transmute::<ITfTextInputProcessorEx, *mut c_void>(
                        TextService::create::<ITfTextInputProcessorEx>()?,
                    )
                }
                _ => {
                    return Err(windows::core::Error::from_hresult(E_NOINTERFACE).into());
                }
            };
        }

        Ok(())
    }

    #[macros::anyhow]
    fn LockServer(&self, flock: BOOL) -> Result<()> {
        let mut dll_instance = DllModule::get()?;
        if flock.into() {
            dll_instance.add_ref();
        } else {
            dll_instance.release();
        }

        Ok(())
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
