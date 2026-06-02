use windows::{
    core::{IUnknown, Interface as _, BSTR, GUID, PCWSTR},
    Win32::{
        Foundation::{BOOL, E_INVALIDARG, POINT, RECT},
        System::Ole::CONNECT_E_CANNOTCONNECT,
        UI::{
            TextServices::{
                ITfLangBarItemButton, ITfLangBarItemButton_Impl, ITfLangBarItemMgr,
                ITfLangBarItemSink, ITfLangBarItem_Impl, ITfMenu, ITfSource_Impl, TfLBIClick,
                GUID_LBI_INPUTMODE, TF_LANGBARITEMINFO, TF_LBI_STYLE_BTN_BUTTON,
            },
            WindowsAndMessaging::{LoadImageW, HICON, IMAGE_ICON, LR_DEFAULTCOLOR},
        },
    },
};

use crate::{
    engine::{
        composition,
        engine_result::EngineResult,
        input_mode::InputMode,
        state::IMEState,
        theme::{get_system_theme, SystemTheme},
    },
    globals::{
        DllModule, GUID_TEXT_SERVICE, IDI_MODE_KANA_BLACK, IDI_MODE_KANA_WHITE,
        IDI_MODE_LATN_BLACK, IDI_MODE_LATN_WHITE, TEXTSERVICE_LANGBARITEMSINK_COOKIE,
    },
};

use anyhow::{Context as _, Result};

use super::text_service::{TextService, TextService_Impl};

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

    #[macros::anyhow]
    fn GetStatus(&self) -> Result<u32> {
        Ok(0)
    }

    #[macros::anyhow]
    fn Show(&self, _f_show: BOOL) -> Result<()> {
        Ok(())
    }

    // this will be shown as a tooltip when you hover the language bar item
    #[macros::anyhow]
    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok(BSTR::default())
    }
}

impl ITfLangBarItemButton_Impl for TextService_Impl {
    #[macros::anyhow]
    fn OnClick(&self, _click: TfLBIClick, _pt: &POINT, _prcarea: *const RECT) -> Result<()> {
        let mode = {
            let ime_mode = &IMEState::get()?.input_mode;
            match ime_mode {
                InputMode::Latin => InputMode::Kana,
                InputMode::Kana => InputMode::Latin,
            }
        };

        {
            let state = IMEState::get()?;
            let mut converter = state.converter.clone().context("converter is None")?;
            let mut candidate_window = state.candidate_window.clone().context("candidate_window is None")?;
            drop(state);
            converter.clear_text()?;
            candidate_window.set_input_mode(match &mode {
                InputMode::Latin => "A",
                InputMode::Kana => "あ",
            })?;
            candidate_window.hide_window()?;
            candidate_window.set_candidates(vec![])?;
        }

        let prev_result = {
            let inner = self.try_borrow()?;
            let prev = inner.current_result.borrow().clone();
            prev
        };

        let result = {
            let inner = self.try_borrow_mut()?;
            let mut comp = inner.borrow_mut_composition()?;
            composition::reset(&mut comp)
        };

        let result = EngineResult {
            next_input_mode: Some(mode),
            ..result
        };

        self.sync_engine_result(prev_result.as_ref(), &result)?;
        {
            let inner = self.try_borrow_mut()?;
            *inner.current_result.borrow_mut() = Some(result);
        }

        Ok(())
    }

    #[macros::anyhow]
    fn InitMenu(&self, _pmenu: Option<&ITfMenu>) -> Result<()> {
        Ok(())
    }

    #[macros::anyhow]
    fn OnMenuSelect(&self, _w_id: u32) -> Result<()> {
        Ok(())
    }

    #[macros::anyhow]
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

    #[macros::anyhow]
    fn GetText(&self) -> Result<BSTR> {
        Ok(BSTR::default())
    }
}

impl ITfSource_Impl for TextService_Impl {
    #[macros::anyhow]
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

impl TextService {
    pub fn update_lang_bar(&self) -> Result<()> {
        let text_service = self.try_borrow()?;
        let thread_mgr = text_service.thread_mgr()?;

        unsafe {
            thread_mgr
                .cast::<ITfLangBarItemMgr>()?
                .RemoveItem(&text_service.this::<ITfLangBarItemButton>()?)?;

            thread_mgr
                .cast::<ITfLangBarItemMgr>()?
                .AddItem(&text_service.this::<ITfLangBarItemButton>()?)?;
        };

        Ok(())
    }
}
