//! 应用图标：把打包进二进制的 PNG 解成 egui 的 `IconData`。
//!
//! 源图是 `assets/icon.png`（1024×1024，圆角底板 + 提交图），
//! 打包 macOS 的 `.app` 时由 `scripts/bundle-macos.sh` 拿同一张图切 `.icns`，
//! 所以**只有一份图标素材**，两边不会长得不一样。

use eframe::egui;

/// 打包时嵌进二进制的源图。
const PNG: &[u8] = include_bytes!("../../assets/icon.png");

/// 给 `ViewportBuilder::with_icon` 用。解不出来就 panic：图标是我们自己打包进来的，
/// 解码失败只可能是素材坏了，静默退回默认图标会让「没有图标」这件事再偷偷发生一次。
pub(crate) fn app_icon() -> egui::IconData {
    decode(PNG).unwrap_or_else(|e| panic!("打包进去的 assets/icon.png 解不出来：{e}"))
}

fn decode(bytes: &[u8]) -> Result<egui::IconData, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    // 只认 8 位 RGBA：源图就是我们自己写出来的，转色反而多一处可能出错的地方。
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| "解码器算不出缓冲大小".to_owned())?;
    let mut rgba = vec![0; size];
    let frame = reader.next_frame(&mut rgba).map_err(|e| e.to_string())?;
    if frame.color_type != png::ColorType::Rgba || frame.bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "图标不是 8 位 RGBA，而是 {:?}/{:?}",
            frame.color_type, frame.bit_depth
        ));
    }
    if frame.width != frame.height {
        return Err(format!("图标不是正方形：{}×{}", frame.width, frame.height));
    }
    if rgba.len() != (frame.width * frame.height * 4) as usize {
        return Err("RGBA 长度与宽高对不上".to_owned());
    }
    Ok(egui::IconData {
        rgba,
        width: frame.width,
        height: frame.height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_icon_decodes_to_a_square_rgba() {
        let icon = decode(PNG).expect("assets/icon.png 应能解码");
        assert_eq!(icon.width, icon.height, "图标必须是正方形");
        assert!(icon.width >= 512, "窗口图标至少 512：{}", icon.width);
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
    }

    /// 四个角必须是透明的：macOS 的 Dock 会把不透明方角整个画出来，圆角就没了。
    #[test]
    fn the_icon_has_transparent_corners() {
        let icon = decode(PNG).expect("assets/icon.png 应能解码");
        let w = icon.width as usize;
        let alpha_at = |x: usize, y: usize| icon.rgba[(y * w + x) * 4 + 3];
        for (x, y) in [(0, 0), (w - 1, 0), (0, w - 1), (w - 1, w - 1)] {
            assert_eq!(alpha_at(x, y), 0, "({x},{y}) 不是透明的");
        }
        // 中心那块要实心，否则说明图画反了。
        assert_eq!(alpha_at(w / 2, w / 2), 255, "图标中心是空的");
    }
}
