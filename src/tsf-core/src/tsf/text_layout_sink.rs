use windows::{
    core::Interface as _,
    Win32::UI::TextServices::{
        ITfContext, ITfContextView, ITfDocumentMgr, ITfSource, ITfTextLayoutSink,
        ITfTextLayoutSink_Impl, TfLayoutCode,
    },
};

use anyhow::Result;

use crate::engine::state::IMEState;

use super::{text_service::TextService_Impl, text_service_inner::TextServiceInner};

impl ITfTextLayoutSink_Impl for TextService_Impl {
    // テキストの位置が変化したこっと期の動作を指定するイベントリスナー的なもの
    // ただし、Windows Storeのアプリ（メモ帳とか）では呼ばれないっぽいので注意が必要
    #[macros::anyhow]
    fn OnLayoutChange(
        &self,
        _pic: Option<&ITfContext>,
        _lcode: TfLayoutCode,
        _pview: Option<&ITfContextView>,
    ) -> Result<()> {
        Ok(())
    }
}

impl TextServiceInner {
    pub fn advise_text_layout_sink(&mut self, doc_mgr: ITfDocumentMgr) -> Result<()> {
        if IMEState::get()?.context.is_some() {
            self.unadvise_text_layout_sink()?;
        }

        unsafe {
            let context = doc_mgr.GetTop()?;

            IMEState::get()?.context = Some(context.clone());

            let cookie = context
                .cast::<ITfSource>()?
                .AdviseSink(&ITfTextLayoutSink::IID, &self.this::<ITfTextLayoutSink>()?)?;

            IMEState::get()?
                .cookies
                .insert(ITfTextLayoutSink::IID, cookie);

            Ok(())
        }
    }

    pub fn unadvise_text_layout_sink(&mut self) -> Result<()> {
        unsafe {
            let mut state = IMEState::get()?;

            if let Some(context) = state.context.take() {
                if let Some(cookie) = state.cookies.remove(&ITfTextLayoutSink::IID) {
                    context.cast::<ITfSource>()?.UnadviseSink(cookie)?;
                }
            }

            Ok(())
        }
    }
}
