use crate::tsf::text_service::TextService_Impl;

use windows::Win32::UI::TextServices::{ITfComposition, ITfCompositionSink_Impl};

impl ITfCompositionSink_Impl for TextService_Impl {
    #[macros::anyhow]
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        composition: Option<&ITfComposition>,
    ) -> Result<()> {
        tracing::debug!("OnCompositionTerminated");
        if let Some(_) = composition {
            let mut inner = self.try_borrow_mut()?;
            if let Some(state) = inner.contexts.active_mut() {
                state.composition = None;
            }
        }
        Ok(())
    }
}
