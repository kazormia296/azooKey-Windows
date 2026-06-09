use std::cell::RefCell;

use anyhow::Result;
use windows::{
    core::{Interface, implement},
    Win32::UI::TextServices::{
        ITfContext, ITfEditSession, ITfEditSession_Impl, ITfInsertAtSelection,
        INSERT_TEXT_AT_SELECTION_FLAGS, TF_ES_READWRITE, TF_ES_SYNC,
    },
};
 
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
    pub fn insert_text(&self, text: &str) -> Result<()> {
        let insert_at: ITfInsertAtSelection = self.context.cast()?;
        let wide = text.to_wide_16_unpadded();
        unsafe {
            // WARNING: INSERT_TEXT_AT_SELECTIONにおいて、TF_IAS_NOQUERYの代わりに0を指定する必要がある
            // NOQUERYを指定すると、返り値のITfRangeの代わりにNULLが返されるが、それをRustがDropしようとしてNull Pointerへのアクセスが発生しクラッシュするする
            insert_at.InsertTextAtSelection(self.ec, INSERT_TEXT_AT_SELECTION_FLAGS(0), &wide)?;
        }

        // insert_textを行う場合、notepad.exeなどのアプリではカーソルの位置が更新されないので、set_selection()でカーソルの位置を変更する必要がある。
        Ok(())
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
            callback(&editor)?;
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
