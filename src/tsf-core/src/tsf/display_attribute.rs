// https://learn.microsoft.com/en-us/windows/win32/tsf/providing-display-attributes

use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};
use windows::{
    Win32::{
        Foundation::{E_FAIL, E_INVALIDARG, S_FALSE},
        UI::TextServices::{
            IEnumTfDisplayAttributeInfo, IEnumTfDisplayAttributeInfo_Impl, ITfDisplayAttributeInfo,
            ITfDisplayAttributeInfo_Impl, ITfDisplayAttributeProvider_Impl, TF_DISPLAYATTRIBUTE,
        },
    }, core::{BSTR, GUID, implement}
};

use crate::globals::{DISPLAY_ATTRIBUTE, GUID_DISPLAY_ATTRIBUTE};

use super::text_service::TextService_Impl;

impl ITfDisplayAttributeProvider_Impl for TextService_Impl {
    #[macros::anyhow(fail_with = E_FAIL)]
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        let enum_info = EnumDisplayAttributeInfo::new(EnumDisplayAttributeInfo::get_display_attributes());
        Ok(enum_info.into())
    }

    #[macros::anyhow(fail_with = E_FAIL)]
    fn GetDisplayAttributeInfo(
        &self,
        guid: *const windows_core::GUID,
    ) -> Result<ITfDisplayAttributeInfo> {
        let guid = unsafe { guid.as_ref() }
            .ok_or_else(|| windows::core::Error::from_hresult(E_INVALIDARG))?;
 
        EnumDisplayAttributeInfo::get_display_attributes()
            .into_iter()
            .find(|attr| attr.guid == *guid)
            .map(|attr| attr.into())
            .ok_or_else(|| windows::core::Error::from_hresult(E_INVALIDARG).into())
    }
}


#[derive(Clone)]
#[implement(ITfDisplayAttributeInfo)]
pub struct DisplayAttributeInfo {
    pub guid: GUID,
    attribute: Cell<TF_DISPLAYATTRIBUTE>,
    attribute_backup: TF_DISPLAYATTRIBUTE,
}

impl DisplayAttributeInfo {
    pub fn new(guid: GUID, attribute: TF_DISPLAYATTRIBUTE) -> Self {
        DisplayAttributeInfo {
            guid,
            attribute: Cell::new(attribute),
            attribute_backup: attribute,
        }
    }
}

impl ITfDisplayAttributeInfo_Impl for DisplayAttributeInfo_Impl {
    #[macros::anyhow]
    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        let pda = unsafe { pda.as_mut() }
            .ok_or_else(|| windows::core::Error::from_hresult(E_INVALIDARG))?;

        *pda = self.attribute.get();

        Ok(())
    }

    #[macros::anyhow(fail_with = E_FAIL)]
    fn GetGUID(&self) -> Result<GUID> {
        Ok(self.guid)
    }

    #[macros::anyhow]
    fn Reset(&self) -> Result<()> {
        self.attribute.set(self.attribute_backup);
        Ok(())
    }

    #[macros::anyhow(fail_with = E_FAIL)]
    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::default())
    }

    #[macros::anyhow]
    fn SetAttributeInfo(&self, pda: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        let pda = unsafe { pda.as_ref() }
            .ok_or_else(|| windows::core::Error::from_hresult(E_INVALIDARG))?;
            
        self.attribute.set(*pda);
        Ok(())
    }
}

#[implement(IEnumTfDisplayAttributeInfo)]
pub struct EnumDisplayAttributeInfo {
    pub attributes: Vec<DisplayAttributeInfo>,
    index: AtomicUsize,
}

impl EnumDisplayAttributeInfo {
    pub fn new(attributes: Vec<DisplayAttributeInfo>) -> Self {
        EnumDisplayAttributeInfo {
            attributes,
            index: AtomicUsize::new(0),
        }
    }

    fn get_display_attributes() -> Vec<DisplayAttributeInfo> {
        vec![
            DisplayAttributeInfo::new(GUID_DISPLAY_ATTRIBUTE, DISPLAY_ATTRIBUTE),
        ]
    }
}

impl IEnumTfDisplayAttributeInfo_Impl for EnumDisplayAttributeInfo_Impl {
    #[macros::anyhow(fail_with = E_FAIL)]
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        let clone = EnumDisplayAttributeInfo {
            attributes: self.attributes.clone(),
            index: AtomicUsize::new(self.index.load(Relaxed)),
        };
        Ok(clone.into())
    }

    #[macros::anyhow]
    fn Next(
        &self,
        ulcount: u32,
        rginfo: *mut Option<ITfDisplayAttributeInfo>,
        pcfetched: *mut u32,
    ) -> Result<()> {
        if ulcount == 0 {
            return Ok(());
        }
        if rginfo.is_null() {
            return Err(windows::core::Error::from_hresult(E_INVALIDARG).into());
        }

        unsafe {
            let dest_slice = std::slice::from_raw_parts_mut(rginfo, ulcount as usize);

            let mut fetched = 0;
            let mut index = self.index.load(Relaxed);

            while fetched < ulcount as usize && index < self.attributes.len() {
                if let (Some(attr), Some(dest)) = (self.attributes.get(index), dest_slice.get_mut(fetched)) {
                    *dest = Some(attr.clone().into());
                    fetched += 1;
                    index += 1;
                }
            }

            self.index.store(index, Relaxed);

            if !pcfetched.is_null() {
                *pcfetched = fetched as u32;
            }

            if fetched < ulcount as usize {
                Err(windows::core::Error::from_hresult(S_FALSE).into())
            } else {
                Ok(())
            }
        }
    }

    #[macros::anyhow]
    fn Reset(&self) -> Result<()> {
        self.index.store(0, Relaxed);
        Ok(())
    }

    #[macros::anyhow]
    fn Skip(&self, ulcount: u32) -> Result<()> {
        let current = self.index.load(Relaxed);
        let target = current + ulcount as usize;
        let limit = self.attributes.len();

        if target <= limit {
            self.index.store(target, Relaxed);
            Ok(())
        } else {
            self.index.store(limit, Relaxed);
            Err(windows::core::Error::from_hresult(S_FALSE).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::TextServices::IEnumTfDisplayAttributeInfo;

    // ヘルパー関数: テスト用のAttributeを複数個作成する
    fn create_test_attributes(count: usize) -> Vec<DisplayAttributeInfo> {
        (0..count)
            .map(|i| {
                // インデックスからGUIDを生成 (例: 先頭の4バイトを i にする)
                let guid = GUID::from_values(i as u32, 0, 0, [0, 0, 0, 0, 0, 0, 0, 0]);
                DisplayAttributeInfo::new(
                    guid,
                    DISPLAY_ATTRIBUTE,
                )
            })
            .collect()
    }

    #[test]
    fn test_enum_display_attribute_info() {
        let attributes = create_test_attributes(3);
        let enum_info: IEnumTfDisplayAttributeInfo = EnumDisplayAttributeInfo::new(attributes.clone()).into();

        // 1回目のNextで2つ取得
        let mut fetched = 0;
        unsafe {
            let mut info_array: [Option<ITfDisplayAttributeInfo>; 2] = [None, None];

            let result = enum_info.Next(&mut info_array, &mut fetched);
            
            assert!(result.is_ok()); // S_OKが返るはず
            assert_eq!(fetched, 2); // 2つ取得できたか確認

            // 取得したAttributeのGUIDが正しいか確認
            assert_eq!(info_array[0].as_ref().unwrap().GetGUID().unwrap(), attributes[0].guid);
            assert_eq!(info_array[1].as_ref().unwrap().GetGUID().unwrap(), attributes[1].guid);
        };

        // 2回目のNextで残りの1つを取得
        unsafe {
            let mut info_array: [Option<ITfDisplayAttributeInfo>; 2] = [None, None];

            let result = enum_info.Next(&mut info_array, &mut fetched);

            assert!(result.is_ok()); // S_FALSE(ok)が返るはず
            assert_eq!(fetched, 1); // 1つだけ取得する
    
            assert_eq!(info_array[0].as_ref().unwrap().GetGUID().unwrap(), attributes[2].guid); // 取得したAttributeのGUIDが正しいか確認
            assert!(info_array[1].is_none()); // 2つ目は取得できないはず
        };

        // Resetして再度取得
        unsafe {
            enum_info.Reset().unwrap();

            let mut info_array: [Option<ITfDisplayAttributeInfo>; 3] = [None, None, None];
            let result = enum_info.Next(&mut info_array, &mut fetched);

            assert!(result.is_ok()); // S_OKが返るはず
            assert_eq!(fetched, 3); // 3つ取得できたか確認

            // 取得したAttributeのGUIDが正しいか確認
            assert_eq!(info_array[0].as_ref().unwrap().GetGUID().unwrap(), attributes[0].guid);
            assert_eq!(info_array[1].as_ref().unwrap().GetGUID().unwrap(), attributes[1].guid);
            assert_eq!(info_array[2].as_ref().unwrap().GetGUID().unwrap(), attributes[2].guid);
        };

        // Skipして取得
        unsafe {
            enum_info.Reset().unwrap();
            enum_info.Skip(1).unwrap(); // 1つスキップ

            let mut info_array: [Option<ITfDisplayAttributeInfo>; 2] = [None, None];
            let result = enum_info.Next(&mut info_array, &mut fetched);
            assert!(result.is_ok()); // S_OKが返るはず
            assert_eq!(fetched, 2); // 2つ取得できたか確認

            // 取得したAttributeのGUIDが正しいか確認
            // 最初の1つはスキップされているはず
            assert_eq!(info_array[0].as_ref().unwrap().GetGUID().unwrap(), attributes[1].guid);
            assert_eq!(info_array[1].as_ref().unwrap().GetGUID().unwrap(), attributes[2].guid);
        }

        // Skipして取得 (スキップしすぎてS_FALSEになるパターン)
        unsafe {
            enum_info.Reset().unwrap();
            let result = enum_info.Skip(5); // 5つスキップ (存在するのは3つだけなので2つスキップしてS_FALSEになるはず)
            assert!(result.is_ok()); // S_FALSE(ok)が返るはず
        }

        // Cloneして取得
        // Cloneしても元の列挙子の状態は変わらないはず
        unsafe {
            enum_info.Reset().unwrap();
            let clone_info = enum_info.Clone().unwrap();

            let mut info_array: [Option<ITfDisplayAttributeInfo>; 3] = [None, None, None];
            let result = clone_info.Next(&mut info_array, &mut fetched);
            assert!(result.is_ok()); // S_OKが返るはず
            assert_eq!(fetched, 3); // 3つ取得できたか確認

            // 取得したAttributeのGUIDが正しいか確認
            assert_eq!(info_array[0].as_ref().unwrap().GetGUID().unwrap(), attributes[0].guid);
            assert_eq!(info_array[1].as_ref().unwrap().GetGUID().unwrap(), attributes[1].guid);
            assert_eq!(info_array[2].as_ref().unwrap().GetGUID().unwrap(), attributes[2].guid);

            // 元の列挙子も状態が変わっていないか確認
            let mut info_array: [Option<ITfDisplayAttributeInfo>; 3] = [None, None, None];
            let result = enum_info.Next(&mut info_array, &mut fetched);
            assert!(result.is_ok()); // S_OKが返るはず
            assert_eq!(fetched, 3); // 3つ取得できたか確認

            // 取得したAttributeのGUIDが正しいか確認
            assert_eq!(info_array[0].as_ref().unwrap().GetGUID().unwrap(), attributes[0].guid);
            assert_eq!(info_array[1].as_ref().unwrap().GetGUID().unwrap(), attributes[1].guid);
            assert_eq!(info_array[2].as_ref().unwrap().GetGUID().unwrap(), attributes[2].guid);
        }
    }
}