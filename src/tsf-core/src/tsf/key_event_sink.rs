use windows::{
    core::GUID,
    Win32::{
        Foundation::{BOOL, LPARAM, WPARAM},
        UI::TextServices::{ITfContext, ITfKeyEventSink_Impl},
    },
};

use super::{edit_session::request_edit_session, text_service::TextService_Impl};

// sink (aka event listener) for key events
// 返り値はS_OKのみであることに注意
impl ITfKeyEventSink_Impl for TextService_Impl {
    #[macros::anyhow(ignore_with = false.into())]
    fn OnTestKeyDown(
        &self,
        _pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(true.into())
    }

    #[macros::anyhow(ignore_with = false.into())]
    fn OnKeyDown(
        &self,
        pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let context = match pic {
            Some(ctx) => ctx,
            None => return Ok(windows::Win32::Foundation::BOOL::from(false)),
        };

        let tid = self.tid.get();
        if tid == 0 {
            return Ok(windows::Win32::Foundation::BOOL::from(false));
        }

        let edit_result = request_edit_session(context, tid, |editor| {
            editor.insert_text("Hello")?;

            Ok(())
        });

        match edit_result {
            Ok(_) => Ok(true.into()),
            Err(e) => {
                // エラーを返す代わりに、falseを返す
                tracing::error!("request_edit_session failed (safe rollback): {:?}", e);
                Ok(false.into())
            }
        }
    }

    #[macros::anyhow(ignore_with = false.into())]
    fn OnTestKeyUp(
        &self,
        _pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        // same as OnTestKeyDown
        Ok(false.into())
    }

    #[macros::anyhow(ignore_with = false.into())]
    fn OnKeyUp(&self, _pic: Option<&ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        // this function is called when a key is released
        // but we handle key events in OnKeyDown function
        // so just return S_OK
        Ok(false.into())
    }

    #[macros::anyhow(ignore_with = false.into())]
    fn OnPreservedKey(&self, _pic: Option<&ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        // this function is actually not used
        Ok(true.into())
    }

    #[macros::anyhow]
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        Ok(())
    }
}
