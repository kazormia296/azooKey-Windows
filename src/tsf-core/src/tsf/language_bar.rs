use windows::{
    core::{BSTR, PCWSTR},
    Win32::{
        Foundation::{BOOL, E_FAIL, E_INVALIDARG, E_NOTIMPL, POINT, RECT},
        UI::{
            TextServices::{
                ITfLangBarItemButton_Impl, ITfLangBarItem_Impl, ITfMenu, TfLBIClick,
                GUID_LBI_INPUTMODE, TF_LANGBARITEMINFO, TF_LBI_ICON, TF_LBI_STYLE_BTN_BUTTON,
                TF_LBI_TEXT, TF_LBI_TOOLTIP,
            },
            WindowsAndMessaging::{LoadImageW, HICON, IMAGE_ICON, LR_DEFAULTCOLOR},
        },
    },
};

use anyhow::Context;

use crate::{
    engine::{
        input_mode::InputMode,
        theme::{get_system_theme, SystemTheme},
    },
    globals::{
        DllModule, GUID_TEXT_SERVICE, IDI_MODE_KANA_BLACK, IDI_MODE_KANA_WHITE,
        IDI_MODE_LATN_BLACK, IDI_MODE_LATN_WHITE,
    },
};

use anyhow::Result;

use super::text_service::{TextService, TextService_Impl};

// https://learn.microsoft.com/en-us/windows/win32/api/ctfutb/ns-ctfutb-tf_langbariteminfo
const LANGUAGE_BAR_INFO: TF_LANGBARITEMINFO = TF_LANGBARITEMINFO {
    // Text ServiceのGUID
    clsidService: GUID_TEXT_SERVICE,
    // GUID_LBI_INPUTMODEしか指定できない（docs参照）
    guidItem: GUID_LBI_INPUTMODE,
    // MEMO: mozcでは言語バーに表示されないバグを防ぐためにTF_LBI_STYLE_SHOWNINTRAYを有効化している
    // https://learn.microsoft.com/en-us/windows/win32/tsf/tf-lbi-style--constants?redirectedfrom=MSDN
    // https://github.com/google/mozc/blob/9f99c20/src/win32/tip/tip_lang_bar_menu.cc#L191-L204
    // ただし、Win10/11の環境ではタスクバーに表示するため、あまり関係がないと思われる
    dwStyle: TF_LBI_STYLE_BTN_BUTTON,
    // 表示の優先度だが、現在のWindowsでは複数のアイテムを表示することができないため、意味がない
    ulSort: 0,
    szDescription: [0; 32],
};

// https://learn.microsoft.com/en-us/windows/win32/api/ctfutb/nn-ctfutb-itflangbaritem
impl ITfLangBarItem_Impl for TextService_Impl {
    #[macros::anyhow]
    fn GetInfo(&self, p_info: *mut TF_LANGBARITEMINFO) -> Result<()> {
        if p_info.is_null() {
            return Err(windows::core::Error::from_hresult(E_INVALIDARG).into());
        }

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
        // TF_LBI_STYLE_HIDDENSTATUSCONTROLを実装していないので、このメソッドは無意味
        // https://learn.microsoft.com/en-us/windows/win32/api/ctfutb/nf-ctfutb-itflangbaritem-show
        Err(windows::core::Error::from_hresult(E_NOTIMPL).into())
    }

    #[macros::anyhow(ignore_with = BSTR::default())]
    fn GetTooltipString(&self) -> Result<BSTR> {
        let tooltip_string = BSTR::from_wide(LANGUAGE_BAR_INFO.szDescription.as_slice())?;
        Ok(tooltip_string)
    }
}

impl ITfLangBarItemButton_Impl for TextService_Impl {
    #[macros::anyhow]
    fn OnClick(&self, _click: TfLBIClick, _pt: &POINT, _prcarea: *const RECT) -> Result<()> {
        // MEMO: TfLBIClickは右クリック(TF_LBI_CLK_RIGHT)、左クリック(TF_LBI_CLK_LEFT)の二つがある
        // https://learn.microsoft.com/en-us/windows/win32/api/ctfutb/ne-ctfutb-tflbiclick
        // TODO: 左クリックでモードの切り替え、右クリックでメニューの表示などを実装する
        Ok(())
    }

    // https://learn.microsoft.com/en-us/windows/win32/api/ctfutb/nn-ctfutb-itflangbaritembutton
    // Remarksにあるように、TF_LBI_STYLE_BTN_BUTTONを指定している場合、このメソッドは呼び出されない
    #[macros::anyhow]
    fn InitMenu(&self, _pmenu: Option<&ITfMenu>) -> Result<()> {
        Ok(())
    }

    // InitMenuと同様に、TF_LBI_STYLE_BTN_BUTTONを指定している場合、このメソッドは呼び出されない
    #[macros::anyhow]
    fn OnMenuSelect(&self, _w_id: u32) -> Result<()> {
        Ok(())
    }

    #[macros::anyhow(fail_with = E_FAIL)]
    fn GetIcon(&self) -> Result<HICON> {
        let dll_module = DllModule::get()?;
        let input_mode = self.input_mode.get();
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
        let text = BSTR::from_wide(LANGUAGE_BAR_INFO.szDescription.as_slice())?;
        Ok(text)
    }
}

impl TextService {
    pub fn update_lang_bar(&self) -> Result<()> {
        if let Some(sink) = self.lang_bar_item_sink.take() {
            unsafe {
                sink.OnUpdate(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_TOOLTIP)?;
            }
            self.lang_bar_item_sink.set(Some(sink));
        }
        Ok(())
    }
}
