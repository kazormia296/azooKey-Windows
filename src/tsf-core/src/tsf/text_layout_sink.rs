use windows::{
    core::Interface as _,
    Win32::UI::TextServices::{
        ITfContext, ITfContextView, ITfDocumentMgr, ITfSource, ITfTextLayoutSink,
        ITfTextLayoutSink_Impl, TfLayoutCode,
    },
};

use anyhow::Result;

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
        for context in self.contexts.iter_mut() {
            context.unadvise_text_layout_sink()?;
        }

        unsafe {
            let context = doc_mgr.GetTop()?;
            let cookie = context
                .cast::<ITfSource>()?
                .AdviseSink(&ITfTextLayoutSink::IID, &self.this::<ITfTextLayoutSink>()?)?;

            if let Some(state) = self.contexts.find_mut(&context) {
                state.text_layout_sink_cookie = Some(cookie);
            }

            Ok(())
        }
    }
}
