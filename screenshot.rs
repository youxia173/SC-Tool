//! 截取游戏所在显示器画面到剪贴板（DXGI 优先，失败回退 GDI）。
//! 同时写入 CF_BITMAP + CF_DIB，兼容 QQ 等软件。

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{CF_BITMAP, CF_DIB};
use windows::Win32::UI::WindowsAndMessaging::*;

pub unsafe fn capture_window_monitor(hwnd: HWND, clipboard_owner: HWND) -> Result<()> {
    let target = if hwnd.0.is_null() {
        GetForegroundWindow()
    } else {
        hwnd
    };
    if target.0.is_null() {
        return Err(Error::from(E_FAIL));
    }

    let (buf, w, h) = match capture_dxgi_pixels(target) {
        Ok(v) => v,
        Err(_) => capture_gdi_pixels(target)?,
    };
    copy_bgra_to_clipboard(clipboard_owner, &buf, w, h)
}

unsafe fn monitor_for_window(hwnd: HWND) -> Result<(RECT, [u16; 32])> {
    let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if !GetMonitorInfoW(hmon, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
        return Err(Error::from(E_FAIL));
    }
    Ok((info.monitorInfo.rcMonitor, info.szDevice))
}

unsafe fn capture_dxgi_pixels(hwnd: HWND) -> Result<(Vec<u8>, u32, u32)> {
    let (_rect, device_name) = monitor_for_window(hwnd)?;
    let factory: IDXGIFactory1 = CreateDXGIFactory1()?;

    for adapter_idx in 0u32..16 {
        let adapter = match factory.EnumAdapters1(adapter_idx) {
            Ok(a) => a,
            Err(_) => break,
        };

        for output_idx in 0u32..8 {
            let output = match adapter.EnumOutputs(output_idx) {
                Ok(o) => o,
                Err(_) => break,
            };

            let desc = output.GetDesc()?;
            if !wide_eq(&desc.DeviceName, &device_name) {
                continue;
            }

            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG(0),
                Some(&[D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
            let device = device.ok_or_else(|| Error::from(E_FAIL))?;
            let context = context.ok_or_else(|| Error::from(E_FAIL))?;

            let output1: IDXGIOutput1 = output.cast()?;
            let dup = output1.DuplicateOutput(&device)?;

            for attempt in 0..12 {
                let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
                let mut resource: Option<IDXGIResource> = None;
                if dup.AcquireNextFrame(500, &mut frame_info, &mut resource).is_err() {
                    continue;
                }
                let Some(resource) = resource else {
                    let _ = dup.ReleaseFrame();
                    continue;
                };

                if attempt < 2 && frame_info.LastPresentTime == 0 {
                    let _ = dup.ReleaseFrame();
                    continue;
                }

                let texture: ID3D11Texture2D = resource.cast()?;
                let mut tex_desc = D3D11_TEXTURE2D_DESC::default();
                texture.GetDesc(&mut tex_desc);

                let staging_desc = D3D11_TEXTURE2D_DESC {
                    Width: tex_desc.Width,
                    Height: tex_desc.Height,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: tex_desc.Format,
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    Usage: D3D11_USAGE_STAGING,
                    BindFlags: 0,
                    CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                    MiscFlags: 0,
                };
                let mut staging: Option<ID3D11Texture2D> = None;
                device.CreateTexture2D(&staging_desc, None, Some(&mut staging))?;
                let staging = staging.ok_or_else(|| Error::from(E_FAIL))?;
                context.CopyResource(&staging, &texture);

                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
                let w = tex_desc.Width as usize;
                let h = tex_desc.Height as usize;
                let pitch = mapped.RowPitch as usize;
                let src = std::slice::from_raw_parts(mapped.pData as *const u8, pitch * h);
                let mut buf = vec![0u8; w * h * 4];
                for y in 0..h {
                    let s = y * pitch;
                    let d = y * w * 4;
                    buf[d..d + w * 4].copy_from_slice(&src[s..s + w * 4]);
                }
                context.Unmap(&staging, 0);
                let _ = dup.ReleaseFrame();
                return Ok((buf, w as u32, h as u32));
            }
        }
    }

    Err(Error::from(E_FAIL))
}

unsafe fn capture_gdi_pixels(hwnd: HWND) -> Result<(Vec<u8>, u32, u32)> {
    let (rect, _) = monitor_for_window(hwnd)?;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return Err(Error::from(E_FAIL));
    }

    let hdc_screen = GetDC(None);
    if hdc_screen.is_invalid() {
        return Err(Error::from(E_FAIL));
    }
    let hdc_mem = CreateCompatibleDC(hdc_screen);
    let hbmp = CreateCompatibleBitmap(hdc_screen, width, height);
    let old = SelectObject(hdc_mem, hbmp);
    let blt_ok = BitBlt(
        hdc_mem,
        0,
        0,
        width,
        height,
        hdc_screen,
        rect.left,
        rect.top,
        SRCCOPY,
    )
    .is_ok();
    let _ = SelectObject(hdc_mem, old);

    if !blt_ok {
        let _ = DeleteObject(hbmp);
        let _ = DeleteDC(hdc_mem);
        let _ = ReleaseDC(None, hdc_screen);
        return Err(Error::from(E_FAIL));
    }

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0 as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut buf = vec![0u8; (width * height * 4) as usize];
    let got = GetDIBits(
        hdc_mem,
        hbmp,
        0,
        height as u32,
        Some(buf.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );

    let _ = DeleteObject(hbmp);
    let _ = DeleteDC(hdc_mem);
    let _ = ReleaseDC(None, hdc_screen);

    if got == 0 {
        return Err(Error::from(E_FAIL));
    }

    Ok((buf, width as u32, height as u32))
}

/// QQ / 微信等更认 CF_BITMAP；同时放 bottom-up CF_DIB 作兜底。
unsafe fn copy_bgra_to_clipboard(owner: HWND, bgra: &[u8], width: u32, height: u32) -> Result<()> {
    let stride = (width * 4) as usize;
    // 转成 bottom-up（正 height），兼容性更好
    let mut bottom_up = vec![0u8; bgra.len()];
    for y in 0..height as usize {
        let src = y * stride;
        let dst = (height as usize - 1 - y) * stride;
        bottom_up[dst..dst + stride].copy_from_slice(&bgra[src..src + stride]);
    }

    let hdc = GetDC(None);
    if hdc.is_invalid() {
        return Err(Error::from(E_FAIL));
    }
    let hbmp = CreateCompatibleBitmap(hdc, width as i32, height as i32);
    if hbmp.is_invalid() {
        let _ = ReleaseDC(None, hdc);
        return Err(Error::from(E_FAIL));
    }

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: height as i32, // bottom-up
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0 as u32,
            biSizeImage: bottom_up.len() as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    let got = SetDIBits(
        hdc,
        hbmp,
        0,
        height,
        bottom_up.as_ptr() as *const _,
        &bmi,
        DIB_RGB_COLORS,
    );
    let _ = ReleaseDC(None, hdc);
    if got == 0 {
        let _ = DeleteObject(hbmp);
        return Err(Error::from(E_FAIL));
    }

    // CF_DIB 内存（BITMAPINFOHEADER + bottom-up 像素）
    let header_size = std::mem::size_of::<BITMAPINFOHEADER>();
    let total = header_size + bottom_up.len();
    let hmem = GlobalAlloc(GMEM_MOVEABLE, total)?;
    let ptr = GlobalLock(hmem) as *mut u8;
    if ptr.is_null() {
        let _ = DeleteObject(hbmp);
        return Err(Error::from(E_FAIL));
    }
    std::ptr::copy_nonoverlapping(&bmi.bmiHeader as *const _ as *const u8, ptr, header_size);
    std::ptr::copy_nonoverlapping(bottom_up.as_ptr(), ptr.add(header_size), bottom_up.len());
    let _ = GlobalUnlock(hmem);

    // 全屏游戏时常抢不到剪贴板：用我们的窗口作 owner，并重试
    let owner = if owner.0.is_null() {
        GetDesktopWindow()
    } else {
        owner
    };
    let mut opened = false;
    for _ in 0..20 {
        if OpenClipboard(owner).is_ok() {
            opened = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if !opened {
        let _ = DeleteObject(hbmp);
        return Err(Error::from(E_FAIL));
    }

    let _ = EmptyClipboard();

    // CF_BITMAP：成功后 hbmp 归系统
    let bmp_ok = SetClipboardData(CF_BITMAP.0 as u32, HANDLE(hbmp.0 as _)).is_ok();
    if !bmp_ok {
        let _ = DeleteObject(hbmp);
    }

    // CF_DIB：成功后 hmem 归系统
    let dib_ok = SetClipboardData(CF_DIB.0 as u32, HANDLE(hmem.0 as _)).is_ok();

    let _ = CloseClipboard();

    if bmp_ok || dib_ok {
        Ok(())
    } else {
        Err(Error::from(E_FAIL))
    }
}

fn wide_eq(a: &[u16], b: &[u16]) -> bool {
    let end_a = a.iter().position(|&c| c == 0).unwrap_or(a.len());
    let end_b = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    if end_a != end_b {
        return false;
    }
    a[..end_a]
        .iter()
        .zip(&b[..end_b])
        .all(|(&x, &y)| wide_ascii_lower(x) == wide_ascii_lower(y))
}

fn wide_ascii_lower(c: u16) -> u16 {
    if (b'A' as u16..=b'Z' as u16).contains(&c) {
        c + 32
    } else {
        c
    }
}
