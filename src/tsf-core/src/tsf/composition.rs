use crate::tsf::text_service::TextService_Impl;

use windows::Win32::UI::TextServices::{ITfComposition, ITfCompositionSink_Impl};

impl ITfCompositionSink_Impl for TextService_Impl {
    #[macros::anyhow]
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        _composition: Option<&ITfComposition>,
    ) -> Result<()> {
        tracing::debug!("OnCompositionTerminated");
        if let Ok(ctx) = self.get_active_context() {
            if let Some(state) = self.contexts.borrow().find(&ctx) {
                state.set_composition(None);
            }
        }
        Ok(())
    }
}
