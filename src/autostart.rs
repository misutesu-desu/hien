//! Starting Hien hidden when the user signs in, through the per-user Run key.

use std::io;

/// Command line flag for a launch that stays hidden until a file is opened.
pub const BACKGROUND_FLAG: &str = "--background";

#[cfg(windows)]
mod imp {
    use std::io;

    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "Hien";

    fn check(code: u32) -> io::Result<()> {
        if code == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(code as i32))
        }
    }

    pub fn is_enabled() -> bool {
        let (key, value) = (wide(RUN_KEY), wide(VALUE));
        let code = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        code == ERROR_SUCCESS
    }

    pub fn set(command: Option<&str>) -> io::Result<()> {
        let (key, value) = (wide(RUN_KEY), wide(VALUE));
        match command {
            Some(command) => {
                let data = wide(command);
                check(unsafe {
                    RegSetKeyValueW(
                        HKEY_CURRENT_USER,
                        key.as_ptr(),
                        value.as_ptr(),
                        REG_SZ,
                        data.as_ptr().cast(),
                        (data.len() * 2) as u32,
                    )
                })
            }
            None => {
                let code =
                    unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) };
                if code == ERROR_FILE_NOT_FOUND {
                    Ok(())
                } else {
                    check(code)
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::io;

    pub fn is_enabled() -> bool {
        false
    }

    pub fn set(_command: Option<&str>) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

pub fn is_enabled() -> bool {
    imp::is_enabled()
}

pub fn enable() -> io::Result<()> {
    let exe = std::env::current_exe()?;
    imp::set(Some(&format!("\"{}\" {BACKGROUND_FLAG}", exe.display())))
}

pub fn disable() -> io::Result<()> {
    imp::set(None)
}
