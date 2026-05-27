use std::{
    cell::{Ref, RefCell, RefMut},
    ffi::c_void,
};

use windows::{
    core::{implement, AsImpl, IUnknown, Interface, GUID},
    Win32::{
        Foundation::{BOOL, E_NOINTERFACE},
        System::Com::{IClassFactory, IClassFactory_Impl},
        UI::TextServices::{
            ITfCompositionSink, ITfDisplayAttributeProvider, ITfKeyEventSink, ITfLangBarItem,
            ITfLangBarItemButton, ITfSource, ITfTextInputProcessor, ITfTextInputProcessorEx,
            ITfTextLayoutSink, ITfThreadMgrEventSink,
        },
    },
};

use anyhow::Result;

use crate::globals::DllModule;

use super::text_service_inner::TextServiceInner;

// TextServiceが実装する必要がある関数を一つの構造体でまとめて実装する。
// TextServiceInnerにはITfContextやCompositionなど実行中に変更するべきグローバル変数が入っているので、
// それらを処理しやすいようにすべてRefCellで包んでinnerにまとめている。
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
#[derive(Debug)]
pub struct TextService {
    inner: RefCell<TextServiceInner>,
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
            return Err(anyhow::Error::new(windows::core::Error::from_hresult(
                E_NOINTERFACE,
            )));
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
                    return Err(anyhow::Error::new(windows::core::Error::from_hresult(
                        E_NOINTERFACE,
                    )))
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
        let factory = Self {
            inner: RefCell::new(TextServiceInner::default()),
        };

        let this = ITfTextInputProcessor::from(factory);
        let factory = unsafe { this.as_impl() };
        factory.borrow_mut()?.this = Some(this.clone());

        unsafe { factory.cast::<I>().map_err(|e| anyhow::Error::new(e)) }
    }

    pub fn borrow_mut(&self) -> Result<RefMut<TextServiceInner>> {
        Ok(self.inner.try_borrow_mut()?)
    }

    pub fn borrow(&self) -> Result<Ref<TextServiceInner>> {
        Ok(self.inner.try_borrow()?)
    }
}
