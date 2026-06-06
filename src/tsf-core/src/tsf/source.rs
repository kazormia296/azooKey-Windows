// https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfsource
use windows::{
    core::{IUnknown, Interface as _, GUID},
    Win32::{
        Foundation::E_INVALIDARG,
        System::Ole::CONNECT_E_CANNOTCONNECT,
        UI::TextServices::{ITfLangBarItemSink, ITfSource_Impl},
    },
};

pub const LANG_BAR_ITEM_SINK_COOKIE: u32 = 0;

use super::text_service::TextService_Impl;

impl ITfSource_Impl for TextService_Impl {
    #[macros::anyhow(fail_with = CONNECT_E_CANNOTCONNECT)]
    fn AdviseSink(&self, riid: *const GUID, punk: Option<&IUnknown>) -> Result<u32> {
        // TODO: punkをどこかに保存する
        if punk.is_none() {
            return Err(windows::core::Error::from_hresult(E_INVALIDARG).into());
        }

        let riid = unsafe { riid.as_ref() }
            .ok_or_else(|| windows::core::Error::from_hresult(E_INVALIDARG))?;

        match *riid {
            ITfLangBarItemSink::IID => Ok(LANG_BAR_ITEM_SINK_COOKIE),
            _ => return Err(windows::core::Error::from_hresult(CONNECT_E_CANNOTCONNECT).into()),
        }
    }

    #[macros::anyhow]
    fn UnadviseSink(&self, dw_cookie: u32) -> Result<()> {
        match dw_cookie {
            LANG_BAR_ITEM_SINK_COOKIE => Ok(()),
            _ => return Err(windows::core::Error::from_hresult(CONNECT_E_CANNOTCONNECT).into()),
        }
    }
}
