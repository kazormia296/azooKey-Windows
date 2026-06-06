use std::collections::HashMap;

use crate::globals::{DllModule, GUID_DISPLAY_ATTRIBUTE};

use super::text_service::TextService_Impl;
use windows::{
    core::Interface as _,
    Win32::{
        Foundation::BOOL,
        System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
        UI::TextServices::{
            CLSID_TF_CategoryMgr, ITfCategoryMgr, ITfKeyEventSink, ITfKeystrokeMgr,
            ITfLangBarItemButton, ITfLangBarItemMgr, ITfSource, ITfTextInputProcessorEx_Impl,
            ITfTextInputProcessor_Impl, ITfThreadMgr, ITfThreadMgrEventSink,
        },
    },
};

use anyhow::Context;

impl ITfTextInputProcessor_Impl for TextService_Impl {
    #[macros::anyhow]
    #[tracing::instrument]
    fn Activate(&self, ptim: Option<&ITfThreadMgr>, tid: u32) -> Result<()> {
        tracing::debug!("Activated with tid: {tid}");

        // add reference to the dll instance to prevent it from being unloaded
        let mut dll_instance = DllModule::get()?;
        dll_instance.add_ref();

        // innerへの借用を最小限にするため、必要なものを先に取得しておく
        let (thread_mgr, this_key_event, this_event_sink, this_lang_bar) = {
            let mut text_service_inner = self.try_borrow_mut()?;

            text_service_inner.tid = tid;
            let thread_mgr = ptim.context("Thread manager is null")?;
            text_service_inner.thread_mgr = Some(thread_mgr.clone());

            let this_key_event = text_service_inner.this::<ITfKeyEventSink>()?;
            let this_event_sink = text_service_inner.this::<ITfThreadMgrEventSink>()?;
            let this_lang_bar = text_service_inner.this::<ITfLangBarItemButton>()?;

            (thread_mgr.clone(), this_key_event, this_event_sink, this_lang_bar)
        };

        tracing::debug!("AdviseKeyEventSink");
        unsafe {
            thread_mgr.cast::<ITfKeystrokeMgr>()?.AdviseKeyEventSink(
                tid,
                &this_key_event,
                BOOL::from(true),
            )?;
        }

        tracing::debug!("AdviseThreadMgrEventSink");
        let thread_mgr_event_sink_cookie = unsafe {
            thread_mgr.cast::<ITfSource>()?.AdviseSink(
                &ITfThreadMgrEventSink::IID,
                &this_event_sink,
            )?
        };

        tracing::debug!("AdviseTextLayoutSink");
        let doc_mgr = unsafe { thread_mgr.GetFocus() };
        if let Ok(doc_mgr) = doc_mgr {
            self.try_borrow_mut()?
                .advise_text_layout_sink(doc_mgr)?;
        }

        tracing::debug!("Initialize langbar");
        unsafe {
            thread_mgr
                .cast::<ITfLangBarItemMgr>()?
                .AddItem(&this_lang_bar)?;
        }

        {
            let mut text_service_inner = self.try_borrow_mut()?;
            text_service_inner.thread_mgr_event_sink_cookie = Some(thread_mgr_event_sink_cookie);

            tracing::debug!("Initialize display attribute");
            let atom_map = unsafe {
                let mut map = HashMap::new();
                let category_mgr: ITfCategoryMgr =
                    CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;

                let atom = category_mgr.RegisterGUID(&GUID_DISPLAY_ATTRIBUTE)?;
                map.insert(GUID_DISPLAY_ATTRIBUTE, atom);
                map
            };

            text_service_inner.display_attribute_atom = atom_map;
        }

        tracing::debug!("Activate success");

        Ok(())
    }

    #[macros::anyhow]
    #[tracing::instrument]
    fn Deactivate(&self) -> Result<()> {
        tracing::debug!("Deactivated");

        // remove reference to the dll instance
        let mut dll_instance = DllModule::get()?;
        dll_instance.release();

        {
            let text_service = self.try_borrow()?;
            let thread_mgr = text_service.thread_mgr()?;

            // remove key event sink
            tracing::debug!("UnadviseKeyEventSink");
            unsafe {
                thread_mgr
                    .cast::<ITfKeystrokeMgr>()?
                    .UnadviseKeyEventSink(text_service.tid)?;
            };

            tracing::debug!("Remove langbar");
            unsafe {
                thread_mgr
                    .cast::<ITfLangBarItemMgr>()?
                    .RemoveItem(&text_service.this::<ITfLangBarItemButton>()?)
            }?;
        }

        let mut text_service = self.try_borrow_mut()?;
        let thread_mgr = text_service.thread_mgr()?;

        // remove thread manager event sink
        tracing::debug!("UnadviseThreadMgrEventSink");
        unsafe {
            if let Some(cookie) = text_service.thread_mgr_event_sink_cookie.take() {
                thread_mgr.cast::<ITfSource>()?.UnadviseSink(cookie)?;
            }
        };

        // drain all contexts (Drop handles sink cleanup automatically)
        tracing::debug!("DropContexts");
        text_service.contexts.drain();

        // clear display attribute
        text_service.display_attribute_atom.clear();

        text_service.tid = 0;
        text_service.thread_mgr = None;

        tracing::debug!("Deactivate success");

        Ok(())
    }
}

impl ITfTextInputProcessorEx_Impl for TextService_Impl {
    #[macros::anyhow]
    fn ActivateEx(&self, ptim: Option<&ITfThreadMgr>, tid: u32, _dwflags: u32) -> Result<()> {
        // called when the text service is activated
        // if this function is implemented, the Activate() function won't be called
        // so we need to call the Activate function manually
        tracing::debug!("Activated(Ex) with tid: {tid}");
        self.Activate(ptim, tid)?;
        Ok(())
    }
}
