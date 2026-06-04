// 言語バーを管理する
// 言語バーとは、右下に表示されるあとかAとかそういうやつ

use windows::{
    core::{IUnknown, Interface as _, BSTR, GUID, PCWSTR},
    Win32::{
        Foundation::{BOOL, E_FAIL, E_INVALIDARG, POINT, RECT},
        System::Ole::CONNECT_E_CANNOTCONNECT,
        UI::{
            TextServices::{
                ITfLangBarItemButton_Impl, ITfLangBarItemSink, ITfLangBarItem_Impl, ITfMenu,
                ITfSource_Impl, TfLBIClick, GUID_LBI_INPUTMODE, TF_LANGBARITEMINFO,
                TF_LBI_STYLE_BTN_BUTTON,
            },
            WindowsAndMessaging::{LoadImageW, HICON, IMAGE_ICON, LR_DEFAULTCOLOR},
        },
    },
};

use anyhow::Context;

use crate::{
    engine::{
        input_mode::InputMode,
        state::IMEState,
        theme::{get_system_theme, SystemTheme},
    },
    globals::{
        DllModule, GUID_TEXT_SERVICE, IDI_MODE_KANA_BLACK, IDI_MODE_KANA_WHITE,
        IDI_MODE_LATN_BLACK, IDI_MODE_LATN_WHITE, TEXTSERVICE_LANGBARITEMSINK_COOKIE,
    },
};

use super::text_service::TextService_Impl;

// https://learn.microsoft.com/en-us/windows/win32/api/ctfutb/ns-ctfutb-tf_langbariteminfo
const LANGUAGE_BAR_INFO: TF_LANGBARITEMINFO = TF_LANGBARITEMINFO {
    clsidService: GUID_TEXT_SERVICE,
    guidItem: GUID_LBI_INPUTMODE,
    dwStyle: TF_LBI_STYLE_BTN_BUTTON,
    ulSort: 0,
    szDescription: [0; 32],
};

// you need to implement these three interfaces to create a language bar item
// if not, you will get E_FAIL error in ITfLangBarItemMgr::AddItem

// 言語バーのアイテム...?
impl ITfLangBarItem_Impl for TextService_Impl {
    #[macros::anyhow]
    fn GetInfo(&self, p_info: *mut TF_LANGBARITEMINFO) -> Result<()> {
        unsafe {
            *p_info = LANGUAGE_BAR_INFO;
        }
        Ok(())
    }

    #[macros::anyhow(ignore_with = 0)]
    fn GetStatus(&self) -> Result<u32> {
        Ok(0)
    }

    #[macros::anyhow]
    fn Show(&self, _f_show: BOOL) -> Result<()> {
        Ok(())
    }

    // this will be shown as a tooltip when you hover the language bar item
    #[macros::anyhow(ignore_with = BSTR::default())]
    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok(BSTR::default())
    }
}

impl ITfLangBarItemButton_Impl for TextService_Impl {
    #[macros::anyhow]
    fn OnClick(&self, _click: TfLBIClick, _pt: &POINT, _prcarea: *const RECT) -> Result<()> {
        Ok(())
    }

    // this method should not be called
    #[macros::anyhow]
    fn InitMenu(&self, _pmenu: Option<&ITfMenu>) -> Result<()> {
        Ok(())
    }

    // this method should not be called
    #[macros::anyhow]
    fn OnMenuSelect(&self, _w_id: u32) -> Result<()> {
        Ok(())
    }

    #[macros::anyhow(fail_with = E_FAIL)]
    fn GetIcon(&self) -> Result<HICON> {
        let dll_module = DllModule::get()?;
        let state = &IMEState::get()?;
        let input_mode = &state.input_mode;
        let theme = get_system_theme()?;

        let icon_id = match input_mode {
            InputMode::Kana => match theme {
                SystemTheme::Light => IDI_MODE_KANA_BLACK,
                SystemTheme::Dark => IDI_MODE_KANA_WHITE,
            },
            InputMode::Latin => match theme {
                SystemTheme::Light => IDI_MODE_LATN_BLACK,
                SystemTheme::Dark => IDI_MODE_LATN_WHITE,
            },
        };

        unsafe {
            let handle = LoadImageW(
                dll_module.hinst.context("Dll instance not found")?,
                PCWSTR(icon_id as *mut u16),
                IMAGE_ICON,
                0,
                0,
                LR_DEFAULTCOLOR,
            )?;

            Ok(HICON(handle.0))
        }
    }

    #[macros::anyhow(ignore_with = BSTR::default())]
    fn GetText(&self) -> Result<BSTR> {
        Ok(BSTR::default())
    }
}

// TODO: ITfSourceはなんでこんなところにあるのか
impl ITfSource_Impl for TextService_Impl {
    #[macros::anyhow(fail_with = E_FAIL)]
    fn AdviseSink(&self, riid: *const GUID, punk: Option<&IUnknown>) -> Result<u32> {
        let riid = unsafe { *riid };

        if riid != ITfLangBarItemSink::IID {
            return Err(anyhow::Error::new(windows_core::Error::from_hresult(
                E_INVALIDARG,
            )));
        }

        if punk.is_none() {
            return Err(anyhow::Error::new(windows_core::Error::from_hresult(
                E_INVALIDARG,
            )));
        }

        Ok(TEXTSERVICE_LANGBARITEMSINK_COOKIE)
    }

    #[macros::anyhow]
    fn UnadviseSink(&self, dw_cookie: u32) -> Result<()> {
        if dw_cookie != TEXTSERVICE_LANGBARITEMSINK_COOKIE {
            return Err(anyhow::Error::new(windows_core::Error::from_hresult(
                CONNECT_E_CANNOTCONNECT,
            )));
        }

        Ok(())
    }
}
