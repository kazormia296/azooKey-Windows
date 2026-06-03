// TOOD: utilsに作って集約したほうがよさそう
use windows::{
    core::w,
    Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, REG_VALUE_TYPE, RRF_RT_REG_DWORD},
};

use anyhow::Result;

// Windowsシステムのテーマ
pub enum SystemTheme {
    Light,
    Dark,
}

pub fn get_system_theme() -> Result<SystemTheme> {
    let mut value_type = REG_VALUE_TYPE::default();
    let mut data = [0u8; 4];
    let mut data_size = data.len() as u32;

    let _ = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            Some(&mut value_type),
            Some(data.as_mut_ptr().cast()),
            Some(&mut data_size),
        )
    };

    let theme = match data[0] {
        0 => SystemTheme::Dark,
        1 => SystemTheme::Light,
        // 0か1以外の値はありえないが、一応Lightを使うようにしておく
        _ => SystemTheme::Light,
    };

    Ok(theme)
}
