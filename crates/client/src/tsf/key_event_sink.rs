/// キーボードのイベントを取得するイベントリスナー
///
/// OnTestKeyDownとOnKeyDown, OnTestKeyUpとOnKeyUpの4種類の関数が存在する。
///
/// OnTestKeyDownでTRUEを返した場合にのみ、OnKeyDownが呼ばれて、TextServiceがキーボードイベントを使ってあれこれできるようになる。
/// このrepoでは、
/// 1. ショートカットキーが押されているとき
/// 2. 漢字変換モードではないとき（つまり英語直接入力モードのとき）
/// はOnTestKeyDownを呼ぶようにしている。
///
/// OnTestKeyUpとOnKeyUpも同様だが、KeyUpをトリガーとしたい操作が今のところ存在しないため、常にFALSEを返すようにしている。
///
/// OnPreservedKeyやOnSetFocusは使われる条件がよくわかっていない。
use windows::{
    core::GUID,
    Win32::{
        Foundation::{BOOL, LPARAM, WPARAM},
        UI::TextServices::{ITfContext, ITfKeyEventSink_Impl},
    },
};

use anyhow::Result;

use super::text_service::TextService_Impl;

// sink (aka event listener) for key events
impl ITfKeyEventSink_Impl for TextService_Impl {
    #[macros::anyhow]
    #[tracing::instrument]
    fn OnTestKeyDown(
        &self,
        pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        // this function checks if the key event will be handled by "OnKeyUp" function
        // so we need to return TRUE if we want to handle the key event
        let result = self.process_key(pic, wparam)?.is_some();

        Ok(result.into())
    }

    #[macros::anyhow]
    #[tracing::instrument]
    fn OnKeyDown(&self, pic: Option<&ITfContext>, wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        // this function is called when a key is pressed
        // we can handle key events here
        let result = self.handle_key(pic, wparam)?;

        Ok(result.into())
    }

    #[macros::anyhow]
    fn OnTestKeyUp(
        &self,
        _pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        // same as OnTestKeyDown
        Ok(false.into())
    }

    #[macros::anyhow]
    fn OnKeyUp(&self, _pic: Option<&ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        // this function is called when a key is released
        // but we handle key events in OnKeyDown function
        // so just return S_OK
        Ok(false.into())
    }

    #[macros::anyhow]
    fn OnPreservedKey(&self, _pic: Option<&ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        // this function is actually not used
        Ok(true.into())
    }

    #[macros::anyhow]
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        Ok(())
    }
}
