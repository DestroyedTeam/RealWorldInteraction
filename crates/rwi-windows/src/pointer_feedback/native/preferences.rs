//! Read-only device preferences. Never writes a user's theme or motion setting.
use crate::pointer_feedback::material::{Style, Theme};
use std::{ffi::c_void, mem::size_of};
use windows::{
    Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
    core::BOOL,
};
use windows::{
    Win32::{Foundation::*, Graphics::Gdi::*, System::Registry::*, UI::WindowsAndMessaging::*},
    core::w,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub(super) struct Preferences {
    pub animate: bool,
    pub theme: Theme,
    pub animation_setting_known: bool,
    pub contrast_setting_known: bool,
    #[serde(skip)]
    pub foreground: [u8; 3],
    #[serde(skip)]
    pub background: [u8; 3],
}
impl Preferences {
    pub fn read() -> Self {
        let mut animation = BOOL(0);
        let animation_setting_known = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some((&mut animation as *mut BOOL).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .is_ok();
        let mut hc = HIGHCONTRASTW {
            cbSize: size_of::<HIGHCONTRASTW>() as u32,
            ..Default::default()
        };
        let contrast_setting_known = unsafe {
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                hc.cbSize,
                Some((&mut hc as *mut HIGHCONTRASTW).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .is_ok();
        let mut light = 1u32;
        let mut bytes = size_of::<u32>() as u32;
        let theme_read = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                w!("AppsUseLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some((&mut light as *mut u32).cast::<c_void>()),
                Some(&mut bytes),
            )
        };
        let theme = if contrast_setting_known && hc.dwFlags.0 & HCF_HIGHCONTRASTON.0 != 0 {
            Theme::HighContrast
        } else if theme_read == ERROR_SUCCESS && light == 0 {
            Theme::Dark
        } else {
            Theme::Light
        };
        let rgb = |c: u32| {
            [
                (c & 255) as u8,
                ((c >> 8) & 255) as u8,
                ((c >> 16) & 255) as u8,
            ]
        };
        Self {
            animate: animation_setting_known && animation.as_bool() && theme != Theme::HighContrast,
            theme,
            animation_setting_known,
            contrast_setting_known,
            foreground: rgb(unsafe { GetSysColor(COLOR_WINDOWTEXT) }),
            background: rgb(unsafe { GetSysColor(COLOR_WINDOW) }),
        }
    }
    pub fn style(self) -> Style {
        Style {
            theme: self.theme,
            foreground: self.foreground,
            background: self.background,
        }
    }
}
