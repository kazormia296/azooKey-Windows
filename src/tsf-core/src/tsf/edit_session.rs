use std::cell::RefCell;

use anyhow::Result;
use windows::{
    core::implement,
    Win32::{
        Foundation::E_FAIL,
        UI::TextServices::{
            ITfContext, ITfEditSession, ITfEditSession_Impl, ITfRange, TF_ANCHOR_START,
            TF_ES_READWRITE, TF_ES_SYNC,
        },
    },
};

// 帰ったとき用のメモ
//
// # EditSessionに必要な関数
// - insert_text (実装済み)
// - delete_backward (実装済み)
// - start_composition
// - set_composition <- ?
// - end_composition
// - set_display_attribute
// // selectionはrangeを伴うので注意！
// - get_selection
// - set_selection
// - collapse_selection
// - get_text_before_text
// - get_text_after_text

use crate::extension::StringExt;

pub struct ContextEditor<'a> {
    context: &'a ITfContext,
    ec: u32,
}

impl<'a> ContextEditor<'a> {
    pub fn new(context: &'a ITfContext, ec: u32) -> Self {
        Self { context, ec }
    }

    /// Compositionを作成せずに文字列をinsertする
    #[macros::anyhow]
    pub fn insert_text(&self, text: &str) -> Result<()> {
        // TSFにはITfInsertAtSelectionというものがあり、それを使ってInsertすることも可能。
        // その場合、以下のようなコードを書くことになる
        //
        // ```rust
        // let insert_at: ITfInsertAtSelection = self.context.cast()?;
        // let wide = text.to_wide_16_unpadded();
        // unsafe {
        //     insert_at.InsertTextAtSelection(self.ec, INSERT_TEXT_AT_SELECTION_FLAGS(0), &wide)?;
        // }
        // ```
        //
        // ただし、InsertTextAtSelectionのフラグとしてTF_IAS_NOQUERYを利用してはならない。
        // TF_IAS_NOQUERYを利用すると、返り値のITfRangeの代わりにnull ptrが返ってくる。
        // それをRustがDropしようとしてアクセス違反でクラッシュしてしまうからである。
        unsafe {
            if let Some(selection) = self.get_selection()? {
                let wide = text.to_wide_16_unpadded();
                selection.SetText(self.ec, 0, &wide)?;
            }
        }
        Ok(())
    }

    #[macros::anyhow]
    pub fn delete_backward(&self, count: i32) -> Result<()> {
        unsafe {
            let selection = match self.get_selection()? {
                Some(sel) => sel,
                None => return Ok(()),
            };

            let range = selection.Clone()?;
            range.Collapse(self.ec, TF_ANCHOR_START)?;

            let mut shifted = 0i32;
            range.ShiftStart(self.ec, -count, &mut shifted, std::ptr::null())?;

            if range.IsEmpty(self.ec)?.as_bool() {
                return Ok(());
            }

            range.SetText(self.ec, 0, &[])?;
        }
        Ok(())
    }

    #[macros::anyhow(fail_with = E_FAIL)]
    pub fn get_selection(&self) -> Result<Option<ITfRange>> {
        unsafe {
            let mut fetched = 0u32;
            let mut selection = [windows::Win32::UI::TextServices::TF_SELECTION::default(); 1];
            self.context
                .GetSelection(self.ec, 0, &mut selection, &mut fetched)?;

            if fetched == 0 {
                return Ok(None);
            }

            let [selection_item] = selection;
            let range = std::mem::ManuallyDrop::into_inner(selection_item.range);

            Ok(range)
        }
    }
}

#[implement(ITfEditSession)]
pub struct EditSession {
    context: ITfContext,
    callback: RefCell<Option<Box<dyn FnOnce(&ContextEditor) -> Result<()>>>>,
}

impl EditSession {
    pub fn new<F>(context: &ITfContext, callback: F) -> Self
    where
        F: FnOnce(&ContextEditor) -> Result<()> + 'static,
    {
        Self {
            context: context.clone(),
            callback: RefCell::new(Some(Box::new(callback))),
        }
    }
}

impl ITfEditSession_Impl for EditSession_Impl {
    #[macros::anyhow]
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        if let Some(callback) = self.callback.borrow_mut().take() {
            let editor = ContextEditor::new(&self.context, ec);
            let _ = callback(&editor);
        }
        Ok(())
    }
}

pub fn request_edit_session<F>(context: &ITfContext, tid: u32, callback: F) -> Result<()>
where
    F: FnOnce(&ContextEditor) -> Result<()> + 'static,
{
    let session = EditSession::new(context, callback);
    let session_interface: ITfEditSession = session.into();
    let flags = TF_ES_READWRITE | TF_ES_SYNC;
    unsafe {
        let hr = context.RequestEditSession(tid, &session_interface, flags)?;
        if hr.is_err() {
            return Err(anyhow::anyhow!(hr));
        }
    }
    Ok(())
}
