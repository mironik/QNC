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
    /// Refreshes per second of its current mode (fields per second when interlaced), as
    /// the OS reports it; None when it does not.
    pub refresh_hz: Option<u32>,
    pub interlaced: bool,
}

impl Screen {
    /// The screen's mode against the source rate: a refresh that is not a whole multiple
    /// of the source rate repeats pictures unevenly (judder). The mode is the user's to
    /// set in the OS; it is never changed here.
    pub fn mode_note(&self, source_fps: f64) -> Option<String> {
        let refresh = f64::from(self.refresh_hz?);
        let ratio = refresh / source_fps;
        let even = ratio >= 1.0 && (ratio - ratio.round()).abs() < 0.01;
        let kind = if self.interlaced { "i" } else { "p" };
        (!even).then(|| {
            format!("HDMI {refresh:.0}{kind}, izvor {source_fps:.2}p: slika ce neravnomjerno trzati, postavi ekran na visekratnik izvora")
        })
    }
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
        EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW, DEVMODEW, DM_INTERLACED,
        ENUM_CURRENT_SETTINGS, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    };

    /// MONITORINFOF_PRIMARY of WinUser.h (windows-sys keeps it in WindowsAndMessaging).
    const MONITORINFOF_PRIMARY: u32 = 1;

    unsafe extern "system" fn found(monitor: HMONITOR, _: HDC, _: *mut RECT, list: LPARAM) -> BOOL {
        let list = unsafe { &mut *(list as *mut Vec<Screen>) };
        let mut info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) } != 0 {
            let rect = info.monitorInfo.rcMonitor;
            // The current mode of that screen's device: refresh and interlacing.
            let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
            mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
            let read = unsafe { EnumDisplaySettingsW(info.szDevice.as_ptr(), ENUM_CURRENT_SETTINGS, &mut mode) } != 0;
            // 0 and 1 mean "the hardware default", not a rate.
            let refresh_hz = (read && mode.dmDisplayFrequency > 1).then_some(mode.dmDisplayFrequency);
            let interlaced = read && unsafe { mode.Anonymous2.dmDisplayFlags } & DM_INTERLACED != 0;
            list.push(Screen {
                x: rect.left,
                y: rect.top,
                width: (rect.right - rect.left).max(0) as u32,
                height: (rect.bottom - rect.top).max(0) as u32,
                primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                refresh_hz,
                interlaced,
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
        Screen { x, y: 0, width, height, primary, refresh_hz: Some(50), interlaced: false }
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

    #[test]
    fn a_refresh_that_is_a_multiple_of_the_source_is_even() {
        let at = |hz, interlaced| Screen { refresh_hz: Some(hz), interlaced, ..screen(0, 1920, 1080, false) };
        assert_eq!(at(50, false).mode_note(50.0), None);
        assert_eq!(at(50, false).mode_note(25.0), None);
        assert_eq!(at(60, false).mode_note(30.0), None);
        assert!(at(60, false).mode_note(50.0).is_some());
        assert!(at(50, false).mode_note(24.0).is_some());
        assert!(at(60, true).mode_note(25.0).unwrap().contains("60i"));
    }

    #[cfg(windows)]
    #[test]
    fn this_computer_has_a_main_screen() {
        let found = screens().unwrap();
        assert!(found.iter().any(|screen| screen.primary));
    }
}
