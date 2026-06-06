use windows::Win32::UI::TextServices::{ITfContext, ITfDocumentMgr, ITfThreadMgrEventSink_Impl};

use super::text_service::TextService_Impl;

// テキストボックスのフォーカスの変更などを取り扱う
impl ITfThreadMgrEventSink_Impl for TextService_Impl {
    #[macros::anyhow]
    fn OnInitDocumentMgr(&self, _pdim: Option<&ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    #[macros::anyhow]
    fn OnUninitDocumentMgr(&self, _pdim: Option<&ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    #[macros::anyhow]
    fn OnSetFocus(
        &self,
        focus: Option<&ITfDocumentMgr>,
        _prevfocus: Option<&ITfDocumentMgr>,
    ) -> Result<()> {
        self.update_lang_bar()?;

        // if focus is changed, the text layout sink should be updated
        if let Some(focus) = focus {
            self.try_borrow_mut()?
                .advise_text_layout_sink(focus.clone())?;
        }
        Ok(())
    }

    #[macros::anyhow]
    fn OnPushContext(&self, pic: Option<&ITfContext>) -> Result<()> {
        if let Some(ctx) = pic {
            self.try_borrow_mut()?.contexts.push(ctx.clone());
        }
        Ok(())
    }

    #[macros::anyhow]
    fn OnPopContext(&self, pic: Option<&ITfContext>) -> Result<()> {
        if let Some(ctx) = pic {
            self.try_borrow_mut()?.contexts.pop(ctx);
        }
        Ok(())
    }
}
