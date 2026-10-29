//! A driver-independent first frame while composition devices are preparing.
//! GDI draws a child HWND, never the flip-model parent's surface. @ref LLP 1149 D6.3.
use super::{Error, Result, Window};
use windows::{
    core::w,
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        Graphics::Gdi::*,
        UI::WindowsAndMessaging::*,
    },
};

pub struct CpuWindow {
    hwnd: HWND,
    bitmap: HBITMAP,
    _parent: Window,
}

impl CpuWindow {
    pub fn new(parent: Window) -> Result<Self> {
        // Disabled children leave input with the carrier. SS_BITMAP's pixels
        // persist through WM_PAINT without a second event loop or window proc.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                None,
                WS_CHILD | WS_DISABLED | WINDOW_STYLE(14), // SS_BITMAP
                0,
                0,
                1,
                1,
                Some(parent.hwnd),
                None,
                None,
                None,
            )?
        };
        Ok(Self {
            hwnd,
            bitmap: HBITMAP::default(),
            _parent: parent,
        })
    }

    pub fn present(&mut self, size: [u32; 2], rgba: &[u8]) -> Result<()> {
        if size.contains(&0)
            || size.iter().any(|n| *n > 16384)
            || rgba.len() as u64 != u64::from(size[0]) * u64::from(size[1]) * 4
        {
            return Err(Error::Invalid("CPU window pixels".into()));
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size[0] as i32,
                biHeight: -(size[1] as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: the DIB owns size.x * size.y pixels; STATIC retains its handle
        // until replacement. Zero alpha avoids STM_SETIMAGE's private alpha copy.
        // https://learn.microsoft.com/windows/win32/controls/stm-setimage
        unsafe {
            let mut bits = std::ptr::null_mut();
            let bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
            for (out, pixel) in std::slice::from_raw_parts_mut(bits.cast::<u8>(), rgba.len())
                .chunks_exact_mut(4)
                .zip(rgba.chunks_exact(4))
            {
                out.copy_from_slice(&[pixel[2], pixel[1], pixel[0], 0]);
            }
            SendMessageW(
                self.hwnd,
                STM_SETIMAGE,
                Some(WPARAM(IMAGE_BITMAP.0 as usize)),
                Some(LPARAM(bitmap.0 as isize)),
            );
            if !self.bitmap.is_invalid() {
                let _ = DeleteObject(self.bitmap.into());
            }
            self.bitmap = bitmap;
            SetWindowPos(
                self.hwnd,
                None,
                0,
                0,
                size[0] as i32,
                size[1] as i32,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
            )?;
            let _ = RedrawWindow(Some(self.hwnd), None, None, RDW_INVALIDATE | RDW_UPDATENOW);
        }
        Ok(())
    }
}

impl Drop for CpuWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
            if !self.bitmap.is_invalid() {
                let _ = DeleteObject(self.bitmap.into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "native Windows bitmap presentation requires a desktop session"]
    fn cpu_child_draws_without_a_device_and_survives_resize() {
        let window = Window::fixture([80, 60]).unwrap();
        let mut child = CpuWindow::new(window.clone()).unwrap();
        for size in [[80, 60], [40, 30], [80, 60]] {
            let bytes = [34, 153, 68, 255].repeat((size[0] * size[1]) as usize);
            child.present(size, &bytes).unwrap();
            window.dispatch_pending();
            unsafe {
                let dc = GetDC(Some(child.hwnd));
                let pixel = GetPixel(dc, (size[0] / 2) as i32, (size[1] / 2) as i32);
                ReleaseDC(Some(child.hwnd), dc);
                assert_eq!(pixel.0, 34 | (153 << 8) | (68 << 16));
            }
        }
    }
}
