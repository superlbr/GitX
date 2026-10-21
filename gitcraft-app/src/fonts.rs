use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::path::PathBuf;
use std::sync::Arc;

/// 判据① 实测（见 PROCESS.md「Spike 结果」）：stock eframe 靠 `system_fonts` 能出汉字和彩色
/// emoji，但 **U+FF1A「：」在 Proportional 链里是豆腐块**，而中文提交信息里最常见的就是这个字符。
/// 所以自带一份全量 CJK 字体不是可选项。用 Noto Sans SC（SIL OFL，可随应用分发）。
const BUNDLED: &str = "NotoSansSC-Regular.otf";

/// 打包件没下载全时的退路：用户自己机器上的单面 CJK 字体。这些**不能**随应用分发，
/// 只够让开发机不出豆腐块。`.ttc` 是字体集合，ab_glyph 解析不了，所以候选里只放单面文件。
const SYSTEM: &[&str] = &["Library/Fonts/msyh.ttf"];

pub fn install(ctx: &egui::Context) {
    let Some((path, bytes)) = load() else {
        eprintln!("gitcraft-app: 没找到可用的 CJK 字体，退回 system_fonts（含「：」的中文会出豆腐块）");
        return;
    };
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("noto-cjk".to_owned(), Arc::new(FontData::from_owned(bytes)));
    // 追加而不是置前：ASCII 仍由链头那个字体出，CJK 才落到 Noto，等宽列的字符宽度不会被带歪。
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("noto-cjk".to_owned());
    }
    ctx.set_fonts(fonts);
    eprintln!("gitcraft-app: CJK 字体用了 {}", path.display());
}

/// 依次试 `GITCRAFT_FONT_DIR`、仓库里的 `assets/fonts`、用户机器上的候选，返回第一个**完整**的。
fn load() -> Option<(PathBuf, Vec<u8>)> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("GITCRAFT_FONT_DIR") {
        candidates.push(PathBuf::from(dir).join(BUNDLED));
    }
    // 从仓库根跑和从 gitcraft-app/ 跑都能命中，开发时不必先 build 资源。
    for root in ["assets/fonts", "gitcraft-app/assets/fonts"] {
        candidates.push(PathBuf::from(root).join(BUNDLED));
    }
    if let Ok(home) = std::env::var("HOME") {
        for rel in SYSTEM {
            candidates.push(PathBuf::from(&home).join(rel));
        }
    }
    for path in candidates {
        // 空文件与半个文件都要在这里挡掉：epaint 解析失败是在不展开的闭包里 panic，
        // 那会让进程直接 abort，而不是降级成豆腐块。
        if !path.exists() {
            continue;
        }
        match std::fs::read(&path) {
            Ok(bytes) if complete(&bytes) => return Some((path, bytes)),
            Ok(bytes) => eprintln!(
                "gitcraft-app: {} 有 {} 字节但表结构不完整（下载被截断？），跳过",
                path.display(),
                bytes.len()
            ),
            Err(error) => eprintln!("gitcraft-app: 读 {} 失败：{error}", path.display()),
        }
    }
    None
}

/// sfnt 头：4 字节版本 + 2 字节 numTables + 10 字节保留，之后每张表一条 16 字节记录
/// （tag / checksum / offset / length，全大端）。下载中断留下的半截文件一定有一张表
/// 越界，而它正是会让 epaint 在不展开的闭包里 panic 的那种输入。
fn complete(bytes: &[u8]) -> bool {
    if bytes.len() < 12 {
        return false;
    }
    let num_tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    if num_tables == 0 || 12 + num_tables * 16 > bytes.len() {
        return false;
    }
    for i in 0..num_tables {
        let record = &bytes[12 + i * 16..12 + i * 16 + 16];
        let offset = u32::from_be_bytes(record[8..12].try_into().unwrap()) as usize;
        let length = u32::from_be_bytes(record[12..16].try_into().unwrap()) as usize;
        if offset + length > bytes.len() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::FontId;

    /// 豆腐块判据：装完字体后，Proportional 链必须真的能出「：」和汉字。
    /// 这条不靠截图——`has_glyph` 问的就是字体链里有没有这个码位，装不装得上、
    /// 装的是哪一份，全都收敛在这一个布尔上。
    #[test]
    fn cjk_font_chain_covers_punctuation_and_hanzi() {
        let Some((_, bytes)) = load() else {
            eprintln!("跳过：这台机器上没有可用的 CJK 字体");
            return;
        };
        let ctx = egui::Context::default();
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert("noto-cjk".to_owned(), Arc::new(FontData::from_owned(bytes)));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .push("noto-cjk".to_owned());
        ctx.set_fonts(fonts);
        // 先跑一帧把字体装进图集：没进图集之前 `has_glyph` 一律是 false，测出来会假红。
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label("：，修交");
        });
        // 无头跑没有后端去应用字体纹理增量，不 clear 掉的话 `out` 落栈时直接 panic。
        out.textures_delta.clear();
        let id = FontId::proportional(12.0);
        for c in ['：', '，', '修', '交'] {
            assert!(ctx.fonts_mut(|f| f.has_glyph(&id, c)), "字体链里没有 {c}");
        }
    }

    /// 反向变异的靶子：把完整字体砍掉最后 4 KB（模拟下载中断），`complete` 必须判 false。
    /// 不判的话这份输入会喂给 epaint，进程直接 abort。
    #[test]
    fn truncated_font_is_rejected() {
        let Some((_, bytes)) = load() else {
            eprintln!("跳过：这台机器上没有可用的 CJK 字体");
            return;
        };
        assert!(complete(&bytes));
        let cut = &bytes[..bytes.len() - 4096];
        assert!(!complete(cut), "砍掉尾部 4KB 还判完整，这道闸等于没有");
        assert!(!complete(&[]));
        assert!(!complete(&[0, 1, 0, 0]));
    }

    /// 空文件、404 页面这类「不是字体」的输入不能进 epaint。
    #[test]
    fn non_font_bytes_are_rejected() {
        assert!(!complete(b"404: Not Found\n"));
        let mut html = b"\x00\x01\x00\x00".to_vec();
        html.extend_from_slice(&[0u8; 4096]);
        // numTables=0 的头，必须挡下
        assert!(!complete(&html));
    }
}
