use std::ffi::c_void;

use windows::{
    Win32::{
        Foundation::{BOOL, CLASS_E_NOAGGREGATION, E_INVALIDARG, E_NOINTERFACE, E_POINTER},
        System::Com::{IClassFactory, IClassFactory_Impl},
        UI::TextServices::{ITfTextInputProcessor, ITfTextInputProcessorEx},
    }, core::{GUID, IUnknown, Interface, implement}
};

use crate::globals::DllModule;

use super::text_service::TextService;

#[derive(Default)]
#[implement(IClassFactory)]
pub struct TextServiceFactory;

impl IClassFactory_Impl for TextServiceFactory_Impl {
    #[macros::anyhow]
    fn CreateInstance(
        &self,
        punkouter: Option<&IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> Result<()> {
        // ppvobjectのチェックと初期化
        if ppvobject.is_null() {
            return Err(windows::core::Error::from_hresult(E_POINTER).into());
        }
        
        unsafe { *ppvobject = std::ptr::null_mut() };

        if riid.is_null() {
            return Err(windows::core::Error::from_hresult(E_INVALIDARG).into());
        }
    
        if punkouter.is_some() {
            return Err(windows::core::Error::from_hresult(CLASS_E_NOAGGREGATION).into());
        }
    
        let riid = unsafe { *riid };
    
        unsafe {
            *ppvobject = match riid {
                IUnknown::IID | ITfTextInputProcessor::IID => {
                    let service = TextService::create::<ITfTextInputProcessor>()?;
                    service.into_raw()
                }
                ITfTextInputProcessorEx::IID => {
                    let service = TextService::create::<ITfTextInputProcessorEx>()?;
                    service.into_raw()
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
