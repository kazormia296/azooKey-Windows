use windows::Win32::UI::TextServices::{ITfContext, ITfDocumentMgr, ITfThreadMgrEventSink_Impl};

use anyhow::Result;

use crate::engine::composition;

use super::text_service::TextService_Impl;

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

        if let Some(focus) = focus {
            self.try_borrow_mut()?.advise_text_layout_sink(focus.clone())?;
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

        self.sync_engine_result(prev_result.as_ref(), &result)?;
        {
            let inner = self.try_borrow_mut()?;
            *inner.current_result.borrow_mut() = Some(result);
        }

        Ok(())
    }

    #[macros::anyhow]
    fn OnPushContext(&self, _pic: Option<&ITfContext>) -> Result<()> {
        Ok(())
    }

    #[macros::anyhow]
    fn OnPopContext(&self, _pic: Option<&ITfContext>) -> Result<()> {
        Ok(())
    }
}
