use crate::tsf::text_service::TextService_Impl;

use anyhow::Result;
use windows::Win32::UI::TextServices::{ITfComposition, ITfCompositionSink_Impl};

impl ITfCompositionSink_Impl for TextService_Impl {
    #[macros::anyhow]
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        _pcomposition: Option<&ITfComposition>,
    ) -> Result<()> {
        // if user clicked outside the composition, the composition will be terminated
        tracing::debug!("OnCompositionTerminated");
        Ok(())
    }
}
