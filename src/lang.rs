//! 界面语言：一档偏好，两处用到它——外壳画字的时候，和核心层替 git 编解释的时候。
//!
//! 为什么是线程局部而不是把 `Lang` 一路当参数传：文案有 400 多处，长在调用点旁边；
//! 而 git 的取数与操作跑在后台线程，错误文案**在那里就拼好了**。进程级 static 更不行——
//! 无头判据在一个进程里并发跑好几个外壳，一个把档位刷成英文，另一个的中文判据就莫名其妙红了。
//! 所以档位跟着线程走：主线程每次 `shell()` 开头刷一次（见 `ui::state::Gitx::shell`），
//! 后台线程只从 [`spawn`] 起——它把调用那一刻的档位复制进新线程，忘了用 `spawn` 的
//! 线程只会退成中文，不会串成另一种语言。

use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    /// 存进 `settings` 的那一格取值。只认这两个，别的读回来当默认档。
    pub fn key(self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::En => "en",
        }
    }

    pub fn from_key(raw: &str) -> Lang {
        match raw {
            "en" => Lang::En,
            _ => Lang::Zh,
        }
    }
}

thread_local! {
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::Zh) };
}

pub fn set(lang: Lang) {
    CURRENT.with(|c| c.set(lang));
}

pub fn get() -> Lang {
    CURRENT.with(Cell::get)
}

/// `tr!` 唯一读的那个判断。写成函数而不是直接读枚举，是为了让宏的两个分支
/// 只在**需要**的时候求值——`format!` 出来的那半边不该白拼一次。
pub fn en() -> bool {
    get() == Lang::En
}

/// 起一个后台线程，并把**调用方此刻**的语言带进去。凡是会产出给人看的文字
/// （git 的错误解释、快照里的状态名）都从这儿起，不用 `std::thread::spawn`。
pub fn spawn<T, F>(f: F) -> std::thread::JoinHandle<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let lang = get();
    std::thread::spawn(move || {
        set(lang);
        f()
    })
}

/// 一个作用域内换档，离开时**必定**还原。
///
/// 无头判据靠它把整个外壳刷成英文再画一帧。手写 `set(En) … set(Zh)` 不行：判据红的时候
/// 是在中间那条 `assert!` 上 panic 出去的，档位就永久留在 EN——之后每条判据看到的都是
/// 英文界面，报出来的失败跟真正的原因毫不相干（而且 EN 判据自己先绿，假绿更难查）。
pub struct Scoped(Lang);

impl Drop for Scoped {
    fn drop(&mut self) {
        set(self.0);
    }
}

pub fn scoped(lang: Lang) -> Scoped {
    let prev = get();
    set(lang);
    Scoped(prev)
}

/// 这段文字里有没有「只有中文文案才会用到的字符」。
///
/// 判据扫漏翻用的就是它，不是 `is_ascii`：英文档的界面本来就会画提交说明、作者名这类
/// 仓库内容（那是数据，翻不了），而它们恰恰是汉字——所以判据得配一份 ASCII 数据的夹具
/// （见 `ui` 那边 `filled_en`），这边只负责认出汉字。
/// 覆盖四块：CJK 标点（「」、，。）、统一表意文字（含扩展 A）、兼容表意文字、
/// 全角形式（（）！？）——漏翻的 `format!("{t}（{n}）")` 就是靠最后那一块露馅的。
pub fn has_han(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        (0x3000..=0x303F).contains(&u)
            || (0x3400..=0x4DBF).contains(&u)
            || (0x4E00..=0x9FFF).contains(&u)
            || (0xF900..=0xFAFF).contains(&u)
            || (0xFF00..=0xFFEF).contains(&u)
    })
}

/// 就地配一对文案，**只算命中的那一支**。
///
/// 不放文案表里是故意的：400 多条 key 表和 400 多个调用点迟早对不上，而且判据想查
/// 「这句英文在哪儿用的」还得反向搜 key。字面量贴在画它的地方，翻没翻一眼看得见，
/// 漏翻由判据 `the_english_shell_paints_no_chinese` 兜住——它扫 EN 档画出来的每一个
/// 码位，不是靠人记住哪一处没改。
#[macro_export]
macro_rules! tr {
    ($zh:expr, $en:expr) => {
        if $crate::lang::en() {
            $en
        } else {
            $zh
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_written_keys_are_english() {
        assert_eq!(Lang::from_key("en"), Lang::En);
        // 读不懂的一律退默认档，跟 `flag()` 那条规矩同源：一条写坏的记录
        // 不该把界面钉在一个谁也没选过的档位上。
        for raw in ["", "EN", "zh", "english", "日本語"] {
            assert_eq!(Lang::from_key(raw), Lang::Zh, "raw={raw:?}");
        }
        assert_eq!(Lang::En.key(), "en");
        assert_eq!(Lang::Zh.key(), "zh");
    }

    #[test]
    fn the_other_thread_keeps_its_own_language() {
        let _g = scoped(Lang::En);
        let raw = std::thread::spawn(|| get()).join().unwrap();
        // 新线程不继承：默认档。继承只走 `spawn`，那是一处看得见的决定。
        assert_eq!(raw, Lang::Zh);
        assert_eq!(get(), Lang::En);
        let copied = spawn(get).join().unwrap();
        assert_eq!(copied, Lang::En);
    }

    #[test]
    fn scoped_gives_the_slot_back_even_when_the_body_panics() {
        set(Lang::Zh);
        // panic 时把红的那条打印咽掉：这条判据要的就是「故意红一次」，刷屏是噪音。
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(|| {
            let _g = scoped(Lang::En);
            assert_eq!(get(), Lang::En);
            panic!("判据在这里红");
        });
        std::panic::set_hook(hook);
        assert!(r.is_err());
        // 手写 `set(En) … set(Zh)` 在这里就漏了：panic 直接跳出，档位留在 EN，
        // 之后每条判据看到的都是英文界面。
        assert_eq!(get(), Lang::Zh, "闭包 panic 后档位没还原");
    }

    #[test]
    fn han_means_chinese_copy_not_just_anything() {
        // 判据的杀伤力全在这个谓词上：认漏了，整条扫描就是假绿。
        for s in ["看板", "「推送」", "（2 个）", "，", "！?", "ＯＫ"] {
            assert!(has_han(s), "{s:?} 里明明有中文标点/汉字，谓词却没认");
        }
        for s in ["Board", "Ready to merge", "—— git output ——", "origin/main", "14px · ▥ ⊙", ""] {
            assert!(!has_han(s), "{s:?} 不是中文文案（破折号、几何符号在两种语言里都用）");
        }
    }

    #[test]
    fn tr_picks_the_branch_for_the_current_language() {
        assert_eq!(tr!("看板", "Board"), "看板");
        let _g = scoped(Lang::En);
        assert_eq!(tr!("看板", "Board"), "Board");
        // 没命中的那一支不能求值：这里让中文那一半在 EN 档下自带计数，
        // 白拼一次 `format!` 就是这么被抓出来的。
        let mut counted = 0;
        let s = tr!(
            {
                counted += 1;
                "中"
            },
            "en"
        );
        assert_eq!(s, "en");
        assert_eq!(counted, 0);
    }
}
