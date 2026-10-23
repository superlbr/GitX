use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::path::PathBuf;
use std::sync::Arc;

/// 界面拉丁字体：Inter 三档字重（SIL OFL，文件在 `assets/fonts/`，随二进制嵌入）。
/// 之前界面只有 egui 自带的 Ubuntu-Light（300）：拉丁正文比中文（Noto 400）和等宽
/// （Hack 400）都细一档，混排一薄一厚；而且全库没有任何真粗体，`strong()` 只能改
/// 取色。三份各 ~400KB，直接进仓库，保证每个构建的拉丁字形都一样。
const INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../../assets/fonts/Inter-Medium.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-SemiBold.ttf");
const INTER: &str = "inter";
const INTER_MEDIUM_FACE: &str = "inter-medium";
const INTER_SEMIBOLD_FACE: &str = "inter-semibold";

/// 标题/强调用的字族：真 Medium。egui 0.37 的 `RichText::strong()` 只改取色不改字重，
/// 要字重只能换 `FontFamily::Name`。族里带 CJK 回退，混排不出豆腐块。
pub fn medium_family() -> FontFamily {
    FontFamily::Name(INTER_MEDIUM_FACE.into())
}

/// 同 [`medium_family`]，更重一档（600），给参考图里明确加粗的行（虚拟行）用。
pub fn semibold_family() -> FontFamily {
    FontFamily::Name(INTER_SEMIBOLD_FACE.into())
}

/// 判据① 实测（见 docs/PROCESS.md「Spike 结果」）：stock eframe 靠 `system_fonts` 能出汉字和彩色
/// emoji，但 **U+FF1A「：」在 Proportional 链里是豆腐块**，而中文提交信息里最常见的就是这个字符。
/// 所以自带一份全量 CJK 字体不是可选项。用 Noto Sans SC（SIL OFL，可随应用分发）。
const BUNDLED: &str = "NotoSansSC-Regular.otf";

/// 打包件没下载全时的退路：用户自己机器上的单面 CJK 字体。这些**不能**随应用分发，
/// 只够让开发机不出豆腐块。`.ttc` 是字体集合，ab_glyph 解析不了，所以候选里只放单面文件。
const SYSTEM: &[&str] = &["Library/Fonts/msyh.ttf"];

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert(INTER.into(), Arc::new(FontData::from_static(INTER_REGULAR)));
    fonts.font_data.insert(
        INTER_MEDIUM_FACE.into(),
        Arc::new(FontData::from_static(INTER_MEDIUM)),
    );
    fonts.font_data.insert(
        INTER_SEMIBOLD_FACE.into(),
        Arc::new(FontData::from_static(INTER_SEMIBOLD)),
    );
    // Inter 置前：拉丁正文从 Ubuntu-Light(300) 换成 Inter(400)，与中文、Hack 同字重。
    // egui 自带的那几个仍留在链尾，兜 Inter 没有的码位（emoji 走 NotoEmoji）。
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, INTER.into());
    fonts
        .families
        .insert(medium_family(), vec![INTER_MEDIUM_FACE.into()]);
    fonts
        .families
        .insert(semibold_family(), vec![INTER_SEMIBOLD_FACE.into()]);
    match load() {
        Some((path, bytes)) => {
            let mut data = FontData::from_owned(bytes);
            // egui 把回退字形的行盒**中心**对到主字体上，不按基线对齐；两份字体的
            // 行盒差半个 (asc+desc) 差 + 半个 lineGap 差。中文和拉丁同一行时基线会
            // 错开几个百分点字号。量两份字体的 hhea 把它抵掉（同 pdfcraft #1676 的做法）。
            if let (Some(primary), Some(fallback)) = (
                vertical_metrics(INTER_REGULAR),
                // epaint 0.37 的 `font` 是 `Blob = Arc<dyn AsRef<[u8]>>`，两层 as_ref 才到字节。
                vertical_metrics(data.font.as_ref().as_ref()),
            ) {
                data.tweak.y_offset_factor = -baseline_drop(primary, fallback);
            }
            fonts
                .font_data
                .insert("noto-cjk".to_owned(), Arc::new(data));
            // 追加而不是置前：ASCII 仍由链头那个字体出，CJK 才落到 Noto，等宽列的
            // 字符宽度不会被带歪。命名字族也补上，否则族内没有的码位直接出豆腐块
            // （CJK 没有真粗体那份，这些场合落 Regular，靠深色补层次）。
            for family in [
                FontFamily::Proportional,
                FontFamily::Monospace,
                medium_family(),
                semibold_family(),
            ] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push("noto-cjk".to_owned());
            }
            ctx.set_fonts(fonts);
            eprintln!("gitx-app: CJK 字体用了 {}", path.display());
        }
        None => {
            ctx.set_fonts(fonts);
            eprintln!("gitx-app: 没找到可用的 CJK 字体，退回 system_fonts（含「：」的中文会出豆腐块）");
        }
    }
}

/// 依次试 `GITX_FONT_DIR`、仓库里的 `assets/fonts`、用户机器上的候选，返回第一个**完整**的。
fn load() -> Option<(PathBuf, Vec<u8>)> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("GITX_FONT_DIR") {
        candidates.push(PathBuf::from(dir).join(BUNDLED));
    }
    // 从仓库根跑和从 gitx-app/ 跑都能命中，开发时不必先 build 资源。
    for root in ["assets/fonts", "gitx-app/assets/fonts"] {
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
                "gitx-app: {} 有 {} 字节但表结构不完整（下载被截断？），跳过",
                path.display(),
                bytes.len()
            ),
            Err(error) => eprintln!("gitx-app: 读 {} 失败：{error}", path.display()),
        }
    }
    None
}

/// sfnt 头：4 字节版本 + 2 字节 numTables + 10 字节保留，之后每张表一条 16 字节记录
/// （tag / checksum / offset / length，全大端）。下载中断留下的半截文件一定有一张表
/// 越界，而它正是会让 epaint 在不展开的闭包里 panic 的那种输入。
fn complete(bytes: &[u8]) -> bool {
    table_range(bytes, b"head").is_some()
}

/// 按 tag 找一张表，返回它在文件里的区间。上面 `complete` 的判据就是「head 表找得到」。
/// 注意**先**全量校验所有表的越界再找 tag：表目录按字母序排，head 在很前面，
/// 找到就返回的话，尾部截断的文件会漏检。
fn table_range(bytes: &[u8], tag: &[u8; 4]) -> Option<(usize, usize)> {
    if bytes.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    if num_tables == 0 || 12 + num_tables * 16 > bytes.len() {
        return None;
    }
    for i in 0..num_tables {
        let record = &bytes[12 + i * 16..12 + i * 16 + 16];
        let offset = u32::from_be_bytes(record[8..12].try_into().unwrap()) as usize;
        let length = u32::from_be_bytes(record[12..16].try_into().unwrap()) as usize;
        if offset + length > bytes.len() {
            return None;
        }
    }
    for i in 0..num_tables {
        let record = &bytes[12 + i * 16..12 + i * 16 + 16];
        if &record[0..4] == tag {
            let offset = u32::from_be_bytes(record[8..12].try_into().unwrap()) as usize;
            let length = u32::from_be_bytes(record[12..16].try_into().unwrap()) as usize;
            return Some((offset, offset + length));
        }
    }
    None
}

/// 一份字体的 hhea 垂直度量，单位是 em：`[ascent, descent(负), lineGap]`。
/// head 表给 unitsPerEm（offset 18），hhea 表给 ascender/descender/lineGap（offset 4/6/8）。
/// 反向变异的靶子在 `truncated_font_is_rejected`。
fn vertical_metrics(bytes: &[u8]) -> Option<[f32; 3]> {
    let head = table_range(bytes, b"head")?;
    if head.1 - head.0 < 20 {
        return None;
    }
    let upm = u16::from_be_bytes([bytes[head.0 + 18], bytes[head.0 + 19]]) as f32;
    if upm <= 0.0 {
        return None;
    }
    let hhea = table_range(bytes, b"hhea")?;
    if hhea.1 - hhea.0 < 10 {
        return None;
    }
    let i16_at = |at: usize| i16::from_be_bytes([bytes[at], bytes[at + 1]]) as f32 / upm;
    Some([
        i16_at(hhea.0 + 4),
        i16_at(hhea.0 + 6),
        i16_at(hhea.0 + 8),
    ])
}

/// 回退字形的基线相对主字体下沉多少（em 为单位，负值是上浮）。egui 居中行盒，
/// 不按基线对齐，所以差值 = 半个 (asc+desc) 之差 + 半个 lineGap 之差
/// （epaint `text_layout`；同 pdfcraft `baseline_drop`）。
fn baseline_drop(primary: [f32; 3], fallback: [f32; 3]) -> f32 {
    let [pa, pd, pg] = primary;
    let [a, d, g] = fallback;
    ((a + d) - (pa + pd) + g - pg) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::FontId;

    /// 嵌进二进制的三份 Inter 必须都是完整 sfnt——它们不走 `load()` 的检查通道，
    /// 坏了只会以 epaint panic 的形式在启动时爆出来，得在这里先挡。
    #[test]
    fn embedded_inter_faces_are_complete_sfnt() {
        for (name, bytes) in [
            ("Inter-Regular", INTER_REGULAR),
            ("Inter-Medium", INTER_MEDIUM),
            ("Inter-SemiBold", INTER_SEMIBOLD),
        ] {
            assert!(complete(bytes), "{name} 不是完整 sfnt");
            assert!(vertical_metrics(bytes).is_some(), "{name} 解不出 hhea 度量");
        }
    }

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

    /// 装配判据：走一遍真的 `install`，Inter 三档都要能被 egui 认出来——命名字族
    /// 拼错、face 没注册这类错，`has_glyph` 在对应族里查不到就是假。
    #[test]
    fn installed_chain_has_inter_at_head_and_weights_resolvable() {
        let ctx = egui::Context::default();
        install(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label("abc 中文");
        });
        out.textures_delta.clear();
        let probes: [(&str, FontFamily, char); 3] = [
            ("Proportional", FontFamily::Proportional, 'a'),
            ("medium", medium_family(), 'b'),
            ("semibold", semibold_family(), 'c'),
        ];
        for (label, family, c) in probes {
            let id = FontId::new(12.0, family.clone());
            assert!(
                ctx.fonts_mut(|f| f.has_glyph(&id, c)),
                "{label} 族里没有 {c}"
            );
        }
        if load().is_some() {
            // CJK 在链尾：命名字族里的汉字也应落到 Noto，而不是豆腐块。
            let id = FontId::new(12.0, semibold_family());
            assert!(ctx.fonts_mut(|f| f.has_glyph(&id, '修')), "命名字族缺 CJK 回退");
        }
    }

    /// 基线对齐量出来的 drop 必须是个小数：两份正常字体的行盒差不该超过半个 em。
    /// 拿 Noto 实测一遍；拿不到 Noto 就用 Inter 自己对自己（必须恰好为 0）。
    #[test]
    fn baseline_drop_stays_within_half_em() {
        let primary = vertical_metrics(INTER_REGULAR).unwrap();
        match load() {
            Some((_, bytes)) => {
                let fallback = vertical_metrics(&bytes).expect("CJK 字体解不出 hhea");
                let drop = baseline_drop(primary, fallback);
                assert!(drop.abs() < 0.5, "基线差 {drop} em，这不像两份正常字体");
            }
            None => assert_eq!(baseline_drop(primary, primary), 0.0),
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
