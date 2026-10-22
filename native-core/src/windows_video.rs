//! Called only by the serialized blocking thumbnail worker, never the UI thread.
use crate::db::Result;
use std::{mem::size_of, os::windows::ffi::OsStrExt, path::Path};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::SIZE,
        Graphics::Gdi::{
            CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
            BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
        },
        System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
        UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_THUMBNAILONLY},
    },
};

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
struct Bitmap(HBITMAP);
impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.0.into());
        }
    }
}
struct DeviceContext(HDC);
impl Drop for DeviceContext {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

pub fn extract(path: &Path, side: u32) -> Result<image::DynamicImage> {
    let unavailable = || "Video thumbnail unavailable. Check the installed video codec.".to_owned();
    let wide = shell_path(path);
    // The path comes only from checked_file, never a shell command or renderer path.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| {
                format!(
                    "Video thumbnail COM initialization failed ({:?}).",
                    e.code()
                )
            })?;
        let _apartment = Apartment;
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)
                .map_err(|e| format!("Video thumbnail item unavailable ({:?}).", e.code()))?;
        // Refuse generic Shell icons: the UI already provides an honest fallback.
        let bitmap = Bitmap(
            factory
                .GetImage(
                    SIZE {
                        cx: side as i32,
                        cy: side as i32,
                    },
                    SIIGBF_THUMBNAILONLY,
                )
                .map_err(|e| format!("Video thumbnail extraction unavailable ({:?}).", e.code()))?,
        );
        let mut metadata = BITMAP::default();
        if GetObjectW(
            bitmap.0.into(),
            size_of::<BITMAP>() as i32,
            Some((&mut metadata as *mut BITMAP).cast()),
        ) != size_of::<BITMAP>() as i32
            || metadata.bmWidth <= 0
            || metadata.bmHeight <= 0
            || metadata.bmWidth > side as i32
            || metadata.bmHeight > side as i32
        {
            return Err(format!(
                "Video thumbnail bitmap dimensions unavailable ({}x{}).",
                metadata.bmWidth, metadata.bmHeight
            ));
        }
        let (width, height) = (metadata.bmWidth as u32, metadata.bmHeight as u32);
        let dc = DeviceContext(CreateCompatibleDC(None));
        if dc.0.is_invalid() {
            return Err(unavailable());
        }
        let mut info = BITMAPINFO::default();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        if GetDIBits(
            dc.0,
            bitmap.0,
            0,
            height,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        ) != height as i32
        {
            return Err(unavailable());
        }
        // GDI BI_RGB is BGRX; the fourth byte is undefined, not transparency.
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        let image = image::RgbaImage::from_raw(width, height, pixels).ok_or_else(unavailable)?;
        Ok(image::DynamicImage::ImageRgba8(image))
    }
}

// Engine containment checks use canonical extended-length paths. The Shell
// parsing API rejects that filesystem namespace, so pass its equivalent Win32
// spelling after validation. Preserve UTF-16 exactly, including UNC paths.
fn shell_path(path: &Path) -> Vec<u16> {
    let encoded: Vec<u16> = path.as_os_str().encode_wide().collect();
    let prefix: Vec<u16> = "\\\\?\\".encode_utf16().collect();
    let unc: Vec<u16> = "UNC\\".encode_utf16().collect();
    let mut result = if let Some(rest) = encoded.strip_prefix(prefix.as_slice()) {
        if let Some(rest) = rest.strip_prefix(unc.as_slice()) {
            "\\\\".encode_utf16().chain(rest.iter().copied()).collect()
        } else {
            rest.to_vec()
        }
    } else {
        encoded
    };
    result.push(0);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shell_spelling_preserves_local_unc_unicode_and_literal_characters() {
        for (source, expected) in [
            (
                r"\\?\C:\selected\ویڈیو ' & $.mp4",
                r"C:\selected\ویڈیو ' & $.mp4",
            ),
            (r"\\?\UNC\server\share\clip.mp4", r"\\server\share\clip.mp4"),
            (r"C:\selected\clip.mp4", r"C:\selected\clip.mp4"),
        ] {
            assert_eq!(
                shell_path(Path::new(source)),
                expected.encode_utf16().chain(Some(0)).collect::<Vec<_>>()
            );
        }
    }
}
