use windows::Win32::UI::TextServices::{ITfComposition, ITfCompositionSink_Impl};

use anyhow::Result;

use crate::engine::composition;

use super::text_service::TextService_Impl;

impl ITfCompositionSink_Impl for TextService_Impl {
    #[macros::anyhow]
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        _pcomposition: Option<&ITfComposition>,
    ) -> Result<()> {
        tracing::debug!("OnCompositionTerminated");

        let result = {
            let inner = self.try_borrow_mut()?;
            let mut comp = inner.composition.borrow_mut();
            composition::reset(&mut comp)
        };

        *self.try_borrow_mut()?.current_result.borrow_mut() = Some(result);

        Ok(())
    }
}
