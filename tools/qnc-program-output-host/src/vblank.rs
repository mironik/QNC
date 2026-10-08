//! The refresh of the output screen, one adapter per OS. A FIFO present alone does not
//! pace the drawing loop to the screen on Windows (a borderless full-screen window gets
//! a queue: two quick draws, then a wait; live 2026-10-08: 72 of 261 draws early), so
//! the newest picture was taken too early and shown twice while the next was dropped.
//! Waiting for the screen's own refresh first takes each picture right after a refresh.

/// Waits for the next refresh of one screen.
pub struct VBlank {
    #[cfg(windows)]
    output: windows::Win32::Graphics::Dxgi::IDXGIOutput,
}

impl VBlank {
    /// The output of the screen at `position` with `size` (desktop pixels); None when
    /// it cannot be found or this OS has no adapter yet (FIFO alone then paces).
    pub fn find(position: (i32, i32), size: (u32, u32)) -> Option<Self> {
        #[cfg(windows)]
        {
            use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
            let middle = (position.0 + size.0 as i32 / 2, position.1 + size.1 as i32 / 2);
            let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.ok()?;
            let mut adapter_index = 0;
            while let Ok(adapter) = unsafe { factory.EnumAdapters1(adapter_index) } {
                let mut output_index = 0;
                while let Ok(output) = unsafe { adapter.EnumOutputs(output_index) } {
                    if let Ok(desc) = unsafe { output.GetDesc() } {
                        let rect = desc.DesktopCoordinates;
                        if (rect.left..rect.right).contains(&middle.0) && (rect.top..rect.bottom).contains(&middle.1) {
                            return Some(Self { output });
                        }
                    }
                    output_index += 1;
                }
                adapter_index += 1;
            }
            None
        }
        #[cfg(not(windows))]
        {
            let _ = (position, size);
            None
        }
    }

    /// Blocks until that screen's next refresh begins.
    pub fn wait(&self) {
        #[cfg(windows)]
        let _ = unsafe { self.output.WaitForVBlank() };
    }
}
