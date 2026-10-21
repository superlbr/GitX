//! 单日 spike：只回答 PROCESS.md 里那两条判据——
//! ① 真实的中文（含 emoji）提交信息能否用 `git log` 原样渲染出不出现豆腐块；
//! ② macOS 拼音能否往多行框里写提交信息（合成态可见、候选窗位置正确、不吞键）。
//! 结论写进 PROCESS.md，这里不作为产品代码维护。
use eframe::egui::{self, FontData, FontDefinitions, RichText};
use std::process::Command;
use std::sync::Arc;

/// 中英数 + 符号 + 单码位 emoji + ZWJ 家庭 emoji，一次全测。
const SAMPLE: &str = "中文测试 提交信息 0.9 ✓ ♠ 改到一半 🎉🚀👍 👨‍👩‍👧";

/// 每个 face 注册成独立家族，截图里直接看出哪个 face 能用、是否支持 .ttc 集合。
///
/// face 数量必须事先核准：epaint 在 `text/family.rs:51` 对解析失败的字体直接
/// `panic!`（且落在不可 unwind 的闭包里），越界 index 会让整个进程死掉，
/// 而不是回退。下面每个 count 都用 CoreText
/// `CTFontManagerCreateFontDescriptorsFromURL` 在这台 macOS 27.0.0 上量过。
///
/// 关键事实：`/System/Library/Fonts/PingFang.ttc` **在这台机器上不存在**。
/// 「PingFang SC」作为家族仍注册，但文件在 PrivateFrameworks 的保留区里。
const FONTS: &[(&str, &str, u32)] = &[
    ("pf", "/System/Library/PrivateFrameworks/FontServices.framework/Resources/Reserved/PingFangUI.ttc", 268),
    ("hg", "/System/Library/Fonts/Hiragino Sans GB.ttc", 4),
    ("ht", "/System/Library/Fonts/STHeiti Light.ttc", 2),
    ("so", "/System/Library/Fonts/Supplemental/Songti.ttc", 8),
    ("em", "/System/Library/Fonts/Apple Color Emoji.ttc", 2),
];

/// pf0=SC Regular、pf3=SC Light、pf4=SC Medium（由 CoreText 的 face 顺序读出）。
const PINGFANG_SC: &[u32] = &[0, 3, 4];

fn face_indices(tag: &str, faces: u32) -> Vec<u32> {
    if tag == "pf" {
        PINGFANG_SC.to_vec()
    } else {
        (0..faces).collect()
    }
}

/// 文件读得到的 face 名，按 FONTS 顺序。矩阵只渲染这份清单：引用没注册的
/// 家族名同样会让 epaint panic。
fn available_faces() -> Vec<String> {
    FONTS
        .iter()
        .filter(|(_, path, _)| std::path::Path::new(path).exists())
        .flat_map(|(tag, _, faces)| face_indices(tag, *faces).into_iter().map(move |i| format!("{tag}{i}")))
        .collect()
}

fn fonts() -> FontDefinitions {
    let mut f = FontDefinitions::default();
    let mut registered: Vec<String> = Vec::new();
    for (tag, path, faces) in FONTS {
        let Ok(bytes) = std::fs::read(path) else { continue };
        let blob: Arc<Vec<u8>> = Arc::new(bytes);
        for index in face_indices(tag, *faces) {
            let name = format!("{tag}{index}");
            let mut data = FontData::from_blob(blob.clone(), index);
            data.tweak.scale = 1.0;
            f.font_data.insert(name.clone(), Arc::new(data));
            f.families
                .insert(egui::FontFamily::Name(name.clone().into()), vec![name.clone()]);
            registered.push(name);
        }
    }
    // 链里只放真正注册成功的面：缺文件时是降级，不是进程死亡。
    let pick = |want: &[&str]| -> Vec<String> {
        want.iter()
            .filter(|w| registered.iter().any(|r| r == *w))
            .map(|w| (*w).to_owned())
            .collect()
    };
    // 两条链对照：有 emoji 回退 vs 没有。
    // pf* 故意排除：截图实测 PingFangUI.ttc 的 face 认 CJK 码位但**画出来是空白**，
    // 比豆腐块更糟（不会触发回退）。
    f.families.insert(
        egui::FontFamily::Name("mono-cjk".into()),
        {
            let mut v = vec!["Hack".into()];
            v.extend(pick(&["hg0", "ht0", "so0", "em0"]));
            v
        },
    );
    f.families.insert(
        egui::FontFamily::Name("mono-cjk-noemoji".into()),
        {
            let mut v = vec!["Hack".into()];
            v.extend(pick(&["hg0", "ht0", "so0"]));
            v
        },
    );
    f.families.insert(
        egui::FontFamily::Name("cjk-only".into()),
        pick(&["pf0", "hg0", "ht0", "so0"]),
    );
    f
}

fn chain(size: f32, family: &str, text: impl Into<String>) -> RichText {
    RichText::new(text).font(egui::FontId::new(
        size,
        egui::FontFamily::Name(family.to_owned().into()),
    ))
}

fn git_log(repo: &str) -> Vec<String> {
    let out = Command::new("git")
        .args(["-C", repo, "log", "-n", "30", "--date=short", "--pretty=%h %ad %an %s"])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).lines().map(|s| s.to_owned()).collect(),
        Err(e) => vec![format!("git 调用失败：{e}")],
    }
}

struct Probe {
    buffer: String,
    log: Vec<String>,
    /// (相对开窗的秒数, 事件)。按时间而不是帧序号驱动：egui 空闲时不重绘，
    /// 用帧计数会把脚本冻在第一帧，测不出任何东西。
    queue: Vec<(f64, egui::Event)>,
    started: std::time::Instant,
    legacy: bool,
    faces: Vec<String>,
    /// 缓冲区实际变化的时刻 —— 判据②的证据就是这几行。
    trace: Vec<String>,
    last_chars: usize,
    /// true = 一个字体都不注册，只测 eframe 默认开启的 `system_fonts` 兜底。
    bare: bool,
}

impl Probe {
    fn new(bare: bool) -> Self {
        let mut p = Self {
            buffer: "改写这次提交的信息：\n".to_owned(),
            log: git_log("/Users/baorong/Developer/superlbr/gitx"),
            queue: Vec::new(),
            started: std::time::Instant::now(),
            legacy: false,
            faces: available_faces(),
            trace: Vec::new(),
            last_chars: 11,
            bare,
        };
        p.inject();
        p
    }

    /// 脚本化的拼音过程：不用人肉切输入法也能复现「合成态 → 上屏」。
    /// 每步之间留够截图窗口。
    fn inject(&mut self) {
        let pre = |at: f64, text: &str, range: Option<std::ops::Range<usize>>| {
            (
                at,
                egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: text.into(),
                    active_range_chars: range,
                }),
            )
        };
        self.started = std::time::Instant::now();
        self.queue.extend([
            pre(0.8, "nihao", Some(0..5)),
            pre(2.2, "你好", Some(0..2)),
            pre(3.6, "你好，世界", Some(2..5)),
            (5.0, egui::Event::Ime(egui::ImeEvent::Commit("你好，世界".into()))),
        ]);
    }
}

impl eframe::App for Probe {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = self.started.elapsed().as_secs_f64();
        // 克隆而不是借用：panel 需要 `&mut ui`，留着 `ui.ctx()` 的引用会冲突。
        let ctx = ui.ctx().clone();
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            ctx.style_mut_of(theme, |s| {
                s.visuals.ime_composition.legacy_visuals = self.legacy
            });
        }

        // 事件必须在 TextEdit 之前塞进本帧的 InputState：begin_pass 每帧用
        // RawInput 重建 events，所以只有同帧、且在控件之前才生效。
        let due: Vec<egui::Event> = self
            .queue
            .iter()
            .filter(|(at, _)| *at <= now)
            .map(|(_, e)| e.clone())
            .collect();
        self.queue.retain(|(at, _)| *at > now);
        if !due.is_empty() {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("commit-box")));
            ctx.input_mut(|i| i.events.extend(due));
        }
        // 脚本没跑完就一直重绘，否则 egui 空闲下来、时间事件永远等不到。
        if !self.queue.is_empty() {
            ctx.request_repaint();
        }

        if self.bare {
            // 给异步解析留机会：前 8 秒每帧重绘。
            if now < 8.0 {
                ctx.request_repaint();
            }
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("bare-box")));
            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading("BARE：一个字体都不注册，只靠 eframe 的 system_fonts 兜底");
                ui.label(format!("t={:.1}s  排队事件={}", now, self.queue.len()));
                ui.label("Proportional：");
                ui.label(RichText::new(SAMPLE).color(ui.visuals().warn_fg_color).size(17.0));
                ui.label("Monospace：");
                ui.label(RichText::new(SAMPLE).monospace().size(17.0));
                ui.label("真实 git log：");
                egui::Frame::side_top_panel(ui.style()).show(ui, |ui| {
                    for line in self.log.iter().take(10) {
                        ui.label(line.clone());
                    }
                    ui.label("f8f56cb 2026-10-16 baorong ✨ 加 emoji：🎉 更名 gitCraft 👨‍👩‍");
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("重放拼音脚本").clicked() {
                        self.trace.clear();
                        self.last_chars = self.buffer.chars().count();
                        self.inject();
                    }
                    ui.checkbox(&mut self.legacy, "旧版合成态视觉");
                });
                let resp = ui.add(
                    egui::TextEdit::multiline(&mut self.buffer)
                        .id(egui::Id::new("bare-box"))
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                let chars = self.buffer.chars().count();
                if chars != self.last_chars {
                    let line = format!(
                        "t={:5.2}s 字数 {:2} → {:2} focused={} 缓冲=「{}」",
                        now,
                        self.last_chars,
                        chars,
                        resp.has_focus(),
                        self.buffer.replace('\n', "\\n")
                    );
                    println!("{line}");
                    self.trace.push(line);
                    self.last_chars = chars;
                }
                ui.label(format!("focused={} 字数={}", resp.has_focus(), chars));
                for t in self.trace.clone() {
                    ui.monospace(t);
                }
            });
            return;
        }

        // ② 放在 TopPanel：屏幕只有 ~900px 高，放在滚动区里会被推到看不见的位置。
        egui::Panel::top("ime").exact_size(340.0).show(ui, |ui| {
            ui.heading("② macOS 拼音：多行提交信息框");
            ui.horizontal(|ui| {
                if ui.button("重放拼音脚本").clicked() {
                    self.trace.clear();
                    self.last_chars = self.buffer.chars().count();
                    self.inject();
                }
                if ui.button("清空").clicked() {
                    self.buffer.clear();
                }
                ui.checkbox(&mut self.legacy, "用旧版合成态视觉（选区式）");
            });
            // 没焦点时注入的 IME 事件会被直接丢掉，脚本期间每帧把焦点抢回来。
            if !self.queue.is_empty() {
                ctx.memory_mut(|m| m.request_focus(egui::Id::new("commit-box")));
            }
            let resp = ui.add(
                egui::TextEdit::multiline(&mut self.buffer)
                    .id(egui::Id::new("commit-box"))
                    .font(egui::FontId::new(
                        15.0,
                        egui::FontFamily::Name("mono-cjk".into()),
                    ))
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            let chars = self.buffer.chars().count();
            if chars != self.last_chars {
                let line = format!(
                    "t={:5.2}s  字数 {:2} → {:2}  focused={}  缓冲=「{}」",
                    now,
                    self.last_chars,
                    chars,
                    resp.has_focus(),
                    self.buffer.replace('\n', "\\n")
                );
                println!("{line}");
                self.trace.push(line);
                self.last_chars = chars;
            }
            ui.label(format!(
                "focused={}  字数={}  排队事件={}  t={:.1}s",
                resp.has_focus(),
                chars,
                self.queue.len(),
                now
            ));
            for t in self.trace.clone() {
                ui.monospace(t);
            }
        });

        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("① 文字渲染：git log 原样显示");

                ui.label("默认 epaint 字体（没注册中文字体时）：");
                ui.label(
                    RichText::new(SAMPLE)
                        .color(ui.visuals().warn_fg_color)
                        .size(17.0),
                );

                ui.add_space(8.0);
                ui.label("系统字体逐个 face（face 数按 CoreText 实测）：");
                self.faces = available_faces();
                for name in &self.faces {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(name.clone()).monospace().size(12.0));
                        ui.label(
                            RichText::new(SAMPLE)
                                .family(egui::FontFamily::Name(name.clone().into()))
                                .size(17.0),
                        );
                    });
                }

                ui.add_space(12.0);
                ui.label("同一条中文链，有无 emoji 回退对照：");
                ui.label(chain(17.0, "mono-cjk", SAMPLE));
                ui.label(chain(17.0, "mono-cjk-noemoji", SAMPLE));

                ui.add_space(12.0);
                ui.label("真实仓库的 git log（mono-cjk 链）：");
                egui::Frame::side_top_panel(ui.style()).show(ui, |ui| {
                    ui.monospace(format!("{} 行", self.log.len()));
                    for line in &self.log {
                        ui.label(chain(13.0, "mono-cjk", line.clone()));
                    }
                    ui.label(
                        chain(
                            13.0,
                            "mono-cjk",
                            "f8f56cb 2026-10-16 baorong ✨ 加 emoji：🎉 更名 gitCraft 👨‍👩‍👧",
                        )
                        .strong(),
                    );
                });
            });
        });
    }
}

fn main() -> eframe::Result {
    // `--bare`：完全不注册任何字体，验证 eframe 默认开的 `system_fonts` 提供者
    // 能不能自己把中文/emoji 兜住。这决定产品要不要自带 CJK 字体。
    let bare = std::env::args().any(|a| a == "--bare");
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 1100.0])
            .with_title(if bare { "uiprobe-bare" } else { "uiprobe" }),
        ..Default::default()
    };
    eframe::run_native(
        if bare { "uiprobe-bare" } else { "uiprobe" },
        opts,
        Box::new(|cc| {
            if !bare {
                cc.egui_ctx.set_fonts(fonts());
            }
            Ok(Box::new(Probe::new(bare)))
        }),
    )
}
