//! The screens of this computer, through one adapter per OS. The output screen is the
//! largest screen that is not the main one; with one screen there is none.

/// A screen in desktop pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Screen {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

/// The screen the program goes to: the largest one that is not the main screen.
pub fn output_screen(screens: &[Screen]) -> Option<Screen> {
    screens
        .iter()
        .filter(|screen| !screen.primary && screen.width > 0 && screen.height > 0)
        .max_by_key(|screen| u64::from(screen.width) * u64::from(screen.height))
        .copied()
}

/// The screens connected now.
#[cfg(windows)]
pub fn screens() -> Result<Vec<Screen>, String> {
    use windows_sys::Win32::Foundation::{BOOL, LPARAM, RECT, TRUE};
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    };

    /// MONITORINFOF_PRIMARY of WinUser.h (windows-sys keeps it in WindowsAndMessaging).
    const MONITORINFOF_PRIMARY: u32 = 1;

    unsafe extern "system" fn found(monitor: HMONITOR, _: HDC, _: *mut RECT, list: LPARAM) -> BOOL {
        let list = unsafe { &mut *(list as *mut Vec<Screen>) };
        let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
            let rect = info.rcMonitor;
            list.push(Screen {
                x: rect.left,
                y: rect.top,
                width: (rect.right - rect.left).max(0) as u32,
                height: (rect.bottom - rect.top).max(0) as u32,
                primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        TRUE
    }

    let mut list: Vec<Screen> = Vec::new();
    let done = unsafe {
        EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(found), &mut list as *mut _ as LPARAM)
    };
    if done == 0 {
        return Err("Popis ekrana nije dostupan.".into());
    }
    Ok(list)
}

/// The screens connected now: no adapter for this OS yet, a controlled answer and no
/// guessed screen (the output stays off).
#[cfg(not(windows))]
pub fn screens() -> Result<Vec<Screen>, String> {
    Err("Program izlaz: popis ekrana na ovom OS-u jos nije podrzan.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(x: i32, width: u32, height: u32, primary: bool) -> Screen {
        Screen { x, y: 0, width, height, primary }
    }

    #[test]
    fn one_screen_gives_no_output() {
        assert_eq!(output_screen(&[screen(0, 1920, 1080, true)]), None);
        assert_eq!(output_screen(&[]), None);
    }

    #[test]
    fn the_largest_screen_that_is_not_the_main_one() {
        let laptop = screen(0, 1920, 1080, true);
        let small = screen(-1280, 1280, 720, false);
        let hdmi = screen(1920, 3840, 2160, false);
        assert_eq!(output_screen(&[laptop, small, hdmi]), Some(hdmi));
        assert_eq!(output_screen(&[small, laptop]), Some(small));
    }

    #[cfg(windows)]
    #[test]
    fn this_computer_has_a_main_screen() {
        let found = screens().unwrap();
        assert!(found.iter().any(|screen| screen.primary));
    }
}
