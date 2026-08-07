//! Hanyu Pinyin (汉语拼音) — tone-mark-stripped Latin romanization.
//!
//! # What is Pinyin?
//!
//! Hanyu Pinyin is the People's Republic of China's official
//! phonetic transcription of Modern Standard Mandarin, adopted in
//! 1958 and codified as **ISO 7098:2015** *Information and
//! documentation — Romanization of Chinese*. Every Han character has
//! one canonical Mandarin reading (some have several — see the
//! [ambiguity](#per-character-ambiguity) note) that Pinyin transcribes
//! as one or more Latin syllables plus a tone number or diacritic:
//!
//! | Han | Pinyin (with tones) | Pinyin (tone-stripped, this crate) |
//! |-----|---------------------|-------------------------------------|
//! | 中  | zhōng               | `zhong`                             |
//! | 国  | guó                 | `guo`                               |
//! | 北  | běi                 | `bei`                               |
//! | 京  | jīng                | `jing`                              |
//! | 你  | nǐ                  | `ni`                                |
//! | 好  | hǎo                 | `hao`                               |
//!
//! # Tone-mark stripping
//!
//! This encoder emits Pinyin **without tone marks**. Tones are a real
//! part of the phonology (`mā` "mother" vs. `mǎ` "horse" vs. `mà`
//! "scold" are four different words with the same segmentally-transcribed
//! syllable `ma`), but a phonetic *key* wants a stable
//! equivalence-class representative for lookup. Tones interfere with
//! that goal in exactly the way an accent-mark strip helps English —
//! `mā` and `ma` end up under the same key, which is the useful
//! behaviour when a query is unsure of the tone.
//!
//! Callers that need tones should carry a separate tone-preserving
//! encoder (a future `pinyin-zh-toned` adapter is a natural
//! extension); this pack ships only the tone-stripped variant.
//!
//! # Coverage
//!
//! The pack ships a **curated ~1000-character table** covering the
//! highest-frequency Han characters in Modern Standard Chinese. At
//! this size the table covers roughly the top 85%+ of running-text
//! character occurrences in a general-purpose modern Chinese corpus
//! — enough to make the encoder useful as a phonetic key generator
//! for common vocabulary without shipping a 20K+ character Unihan
//! table (which would be several hundred KB of data and would push
//! the crate past the wasm-first size budget).
//!
//! Unknown Han characters — those outside the shipped table — encode
//! as the single-byte placeholder `?`. This keeps the encoder's
//! output ASCII (a useful key property) and makes coverage gaps
//! visible in the encoded key.
//!
//! # Per-character ambiguity
//!
//! Many Han characters have more than one Mandarin reading (called
//! *polyphones*, 多音字):
//!
//! - `了` reads `le` (aspect particle: `吃了` "ate") or `liǎo`
//!   ("understand", "finish": `了解` "understand").
//! - `地` reads `de` (adverbial particle: `慢慢地` "slowly") or `dì`
//!   ("earth", "ground": `土地` "land").
//! - `长` reads `cháng` ("long": `长城` "Great Wall") or `zhǎng`
//!   ("grow / elder": `长大` "grow up").
//! - `重` reads `zhòng` ("heavy": `重要` "important") or `chóng`
//!   ("again": `重复` "repeat").
//!
//! Dictionary-level disambiguation needs a context-aware analyzer
//! (jieba / thulac / `HanLP`) that this pack deliberately does not
//! ship. The table entry for a polyphonic character records the
//! **most-frequent reading** by running-text corpus frequency; a
//! future context-aware wrapper (`stringcheese-zh-jieba`, deferred)
//! could override with per-word readings.
//!
//! # Non-goals
//!
//! - **Bopomofo / Zhuyin.** Taiwan uses Zhuyin (注音符号,
//!   U+3100..=U+312F) as its native phonetic notation. Not shipped.
//! - **Wade-Giles.** The pre-Pinyin Latin romanization still used by
//!   some proper-noun conventions (`Peking` for `北京 Beijing`;
//!   `Chiang Kai-shek` for `蒋介石 Jiǎng Jièshí`). Not shipped.
//! - **Yale, Tongyong Pinyin, Gwoyeu Romatzyh.** Historical
//!   alternatives to Hanyu Pinyin. Not shipped.
//! - **Cantonese Jyutping.** Cantonese has its own phonology,
//!   romanization (Jyutping / Yale), and pack (deferred:
//!   `stringcheese-yue-jyutping`).
//! - **Traditional-character coverage.** The shipped table is keyed
//!   on Simplified surface forms with a small number of common
//!   Traditional entries included as a convenience. Traditional
//!   characters that share a Simplified equivalent (e.g. `國` for
//!   `国`) will typically not be found in the table; a caller who
//!   needs Traditional support should compose an S↔T converter or
//!   use a `stringcheese-zh-hant` sibling (deferred).

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

use crate::tokenizer::{CharType, classify};

/// The Hanyu Pinyin encoder (tone-mark-stripped).
///
/// A zero-sized value; construct as [`Pinyin`] and reuse the value
/// freely across threads and calls.
///
/// See the [module-level docs](self) for the coverage details,
/// polyphonic-character policy, and non-goals.
///
/// # Example
///
/// ```
/// use stringcheese_zh::Pinyin;
///
/// assert_eq!(Pinyin.encode("你好"), "ni hao");
/// assert_eq!(Pinyin.encode("中国"), "zhong guo");
/// assert_eq!(Pinyin.encode("北京"), "bei jing");
/// // ASCII passes through unchanged (lowercased).
/// assert_eq!(Pinyin.encode("Hello"), "hello");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pinyin;

impl Pinyin {
    /// Encode `text` as tone-stripped Hanyu Pinyin.
    ///
    /// - Each Han character in the shipped table becomes its Pinyin
    ///   syllable.
    /// - Each Han character *not* in the table becomes `?`.
    /// - ASCII letters pass through, lowercased.
    /// - Digits pass through as-is.
    /// - Whitespace and punctuation act as separators; consecutive
    ///   separators collapse.
    ///
    /// Emitted tokens are joined with a single ASCII space between
    /// units — each Han character is its own space-separated unit;
    /// runs of Latin letters or digits stay together as one unit.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        // Rough capacity: most syllables are 3-5 ASCII bytes.
        let mut out = String::with_capacity(text.len());
        let mut prev_was_token = false;
        // Track whether the previous "unit" was a Han character (each
        // Han emits as its own space-separated syllable), or a run of
        // ASCII letters / digits (which stay together).
        let mut in_run = false;

        for c in text.chars() {
            let ct = classify(c);
            match ct {
                CharType::Han => {
                    if prev_was_token {
                        out.push(' ');
                    }
                    match lookup(c) {
                        Some(py) => out.push_str(py),
                        None => out.push('?'),
                    }
                    prev_was_token = true;
                    in_run = false;
                }
                CharType::Alphabet => {
                    // Continue a Latin run without inserting a space
                    // between letters.
                    if prev_was_token && !in_run {
                        out.push(' ');
                    }
                    push_ascii_lower(&mut out, c);
                    prev_was_token = true;
                    in_run = true;
                }
                CharType::Digit => {
                    if prev_was_token && !in_run {
                        out.push(' ');
                    }
                    // Fold full-width digits to ASCII.
                    let d = if c.is_ascii_digit() {
                        c
                    } else {
                        // U+FF10..=U+FF19 → '0'..='9'.
                        let offset = c as u32 - 0xFF10;
                        char::from_u32(u32::from(b'0') + offset).unwrap_or('0')
                    };
                    out.push(d);
                    prev_was_token = true;
                    in_run = true;
                }
                CharType::Whitespace | CharType::Punctuation => {
                    // Separator — closes any in-progress run. Do not
                    // emit anything; the next token-worthy scalar
                    // will insert its own leading space if needed.
                    in_run = false;
                }
                CharType::Other => {
                    // Non-Han non-Latin scalar (emoji, dingbat,
                    // other-script letter). Pass through as-is,
                    // treated like a Han unit (its own
                    // space-separated token) since we don't know how
                    // to romanize it.
                    if prev_was_token {
                        out.push(' ');
                    }
                    out.push(c);
                    prev_was_token = true;
                    in_run = false;
                }
            }
        }
        out
    }
}

/// Push `c` lowercased into `out`, folding full-width Latin letters
/// (U+FF21..=U+FF3A, U+FF41..=U+FF5A) to ASCII lowercase.
fn push_ascii_lower(out: &mut String, c: char) {
    if c.is_ascii() {
        out.push(c.to_ascii_lowercase());
        return;
    }
    // Full-width uppercase Ａ-Ｚ (U+FF21..=U+FF3A) → 'a'..='z'.
    if ('\u{FF21}'..='\u{FF3A}').contains(&c) {
        let offset = c as u32 - 0xFF21;
        let lower = char::from_u32(u32::from(b'a') + offset).unwrap_or('a');
        out.push(lower);
        return;
    }
    // Full-width lowercase ａ-ｚ (U+FF41..=U+FF5A) → 'a'..='z'.
    if ('\u{FF41}'..='\u{FF5A}').contains(&c) {
        let offset = c as u32 - 0xFF41;
        let lower = char::from_u32(u32::from(b'a') + offset).unwrap_or('a');
        out.push(lower);
        return;
    }
    out.push(c);
}

/// Look up `c` in the shipped Han → Pinyin table. Returns `None` if
/// the character is not in the table.
#[inline]
fn lookup(c: char) -> Option<&'static str> {
    // A binary search over the sorted-by-scalar-value table. The
    // table is a `&[(char, &str)]` of ~1000 entries.
    PINYIN_TABLE
        .binary_search_by_key(&c, |&(k, _)| k)
        .ok()
        .map(|i| PINYIN_TABLE[i].1)
}

/// Number of entries in the shipped Han → Pinyin table.
///
/// Exposed so callers can programmatically report coverage.
pub const PINYIN_TABLE_LEN: usize = PINYIN_TABLE.len();

/// Han → Pinyin (tone-stripped) mapping.
///
/// **Sorted by Han scalar value** — the [`lookup`] function relies on
/// this ordering for its binary search. If you add a new entry,
/// insert it at the sorted position (the `table_is_sorted_by_scalar_value`
/// test will trip if you don't).
///
/// ~1000 entries covering the highest-frequency Han characters in
/// Modern Standard Chinese (drawn from Jun Da's *Modern Chinese
/// Character Frequency List* and comparable corpora). Coverage
/// rationale is documented at the module level. Polyphonic characters
/// (`多音字`) record the most-frequent reading; see the module-level
/// `Per-character ambiguity` note.
///
/// Sourcing note: readings were cross-checked against the CC-CEDICT
/// (Creative Commons–licensed) and Unihan `kMandarin` fields.
static PINYIN_TABLE: &[(char, &str)] = &[
    ('\u{4E00}', "yi"),     // 一
    ('\u{4E03}', "qi"),     // 七
    ('\u{4E07}', "wan"),    // 万
    ('\u{4E08}', "zhang"),  // 丈
    ('\u{4E09}', "san"),    // 三
    ('\u{4E0A}', "shang"),  // 上
    ('\u{4E0B}', "xia"),    // 下
    ('\u{4E0D}', "bu"),     // 不
    ('\u{4E0E}', "yu"),     // 与
    ('\u{4E11}', "chou"),   // 丑
    ('\u{4E13}', "zhuan"),  // 专
    ('\u{4E14}', "qie"),    // 且
    ('\u{4E16}', "shi"),    // 世
    ('\u{4E1A}', "ye"),     // 业
    ('\u{4E1C}', "dong"),   // 东
    ('\u{4E24}', "liang"),  // 两
    ('\u{4E25}', "yan"),    // 严
    ('\u{4E2A}', "ge"),     // 个
    ('\u{4E2D}', "zhong"),  // 中
    ('\u{4E30}', "feng"),   // 丰
    ('\u{4E32}', "chuan"),  // 串
    ('\u{4E3B}', "zhu"),    // 主
    ('\u{4E3E}', "ju"),     // 举
    ('\u{4E48}', "me"),     // 么
    ('\u{4E49}', "yi"),     // 义
    ('\u{4E4B}', "zhi"),    // 之
    ('\u{4E4C}', "wu"),     // 乌
    ('\u{4E4E}', "hu"),     // 乎
    ('\u{4E4F}', "fa"),     // 乏
    ('\u{4E50}', "le"),     // 乐
    ('\u{4E5D}', "jiu"),    // 九
    ('\u{4E5F}', "ye"),     // 也
    ('\u{4E60}', "xi"),     // 习
    ('\u{4E61}', "xiang"),  // 乡
    ('\u{4E66}', "shu"),    // 书
    ('\u{4E70}', "mai"),    // 买
    ('\u{4E71}', "luan"),   // 乱
    ('\u{4E86}', "le"),     // 了
    ('\u{4E88}', "yu"),     // 予
    ('\u{4E8B}', "shi"),    // 事
    ('\u{4E8C}', "er"),     // 二
    ('\u{4E8E}', "yu"),     // 于
    ('\u{4E91}', "yun"),    // 云
    ('\u{4E92}', "hu"),     // 互
    ('\u{4E94}', "wu"),     // 五
    ('\u{4E95}', "jing"),   // 井
    ('\u{4E9B}', "xie"),    // 些
    ('\u{4EA1}', "wang"),   // 亡
    ('\u{4EA4}', "jiao"),   // 交
    ('\u{4EA6}', "yi"),     // 亦
    ('\u{4EA7}', "chan"),   // 产
    ('\u{4EAB}', "xiang"),  // 享
    ('\u{4EAC}', "jing"),   // 京
    ('\u{4EAE}', "liang"),  // 亮
    ('\u{4EB2}', "qin"),    // 亲
    ('\u{4EBA}', "ren"),    // 人
    ('\u{4EBF}', "yi"),     // 亿
    ('\u{4EC0}', "shen"),   // 什
    ('\u{4EC1}', "ren"),    // 仁
    ('\u{4EC5}', "jin"),    // 仅
    ('\u{4EC6}', "pu"),     // 仆
    ('\u{4EC7}', "chou"),   // 仇
    ('\u{4ECA}', "jin"),    // 今
    ('\u{4ECB}', "jie"),    // 介
    ('\u{4ECD}', "reng"),   // 仍
    ('\u{4ECE}', "cong"),   // 从
    ('\u{4ED6}', "ta"),     // 他
    ('\u{4ED8}', "fu"),     // 付
    ('\u{4EE3}', "dai"),    // 代
    ('\u{4EE4}', "ling"),   // 令
    ('\u{4EE5}', "yi"),     // 以
    ('\u{4EEC}', "men"),    // 们
    ('\u{4EF6}', "jian"),   // 件
    ('\u{4EFB}', "ren"),    // 任
    ('\u{4EFD}', "fen"),    // 份
    ('\u{4EFF}', "fang"),   // 仿
    ('\u{4F01}', "qi"),     // 企
    ('\u{4F0D}', "wu"),     // 伍
    ('\u{4F11}', "xiu"),    // 休
    ('\u{4F17}', "zhong"),  // 众
    ('\u{4F18}', "you"),    // 优
    ('\u{4F1A}', "hui"),    // 会
    ('\u{4F20}', "chuan"),  // 传
    ('\u{4F24}', "shang"),  // 伤
    ('\u{4F26}', "lun"),    // 伦
    ('\u{4F2A}', "wei"),    // 伪
    ('\u{4F30}', "gu"),     // 估
    ('\u{4F34}', "ban"),    // 伴
    ('\u{4F38}', "shen"),   // 伸
    ('\u{4F3C}', "si"),     // 似
    ('\u{4F46}', "dan"),    // 但
    ('\u{4F4D}', "wei"),    // 位
    ('\u{4F4E}', "di"),     // 低
    ('\u{4F4F}', "zhu"),    // 住
    ('\u{4F50}', "zuo"),    // 佐
    ('\u{4F53}', "ti"),     // 体
    ('\u{4F55}', "he"),     // 何
    ('\u{4F59}', "yu"),     // 余
    ('\u{4F5B}', "fo"),     // 佛
    ('\u{4F5C}', "zuo"),    // 作
    ('\u{4F60}', "ni"),     // 你
    ('\u{4F73}', "jia"),    // 佳
    ('\u{4F7F}', "shi"),    // 使
    ('\u{4F8B}', "li"),     // 例
    ('\u{4F9B}', "gong"),   // 供
    ('\u{4F9D}', "yi"),     // 依
    ('\u{4FA0}', "xia"),    // 侠
    ('\u{4FA6}', "zhen"),   // 侦
    ('\u{4FA7}', "ce"),     // 侧
    ('\u{4FAF}', "hou"),    // 侯
    ('\u{4FB5}', "qin"),    // 侵
    ('\u{4FBF}', "bian"),   // 便
    ('\u{4FC3}', "cu"),     // 促
    ('\u{4FC4}', "e"),      // 俄
    ('\u{4FD7}', "su"),     // 俗
    ('\u{4FDD}', "bao"),    // 保
    ('\u{4FE1}', "xin"),    // 信
    ('\u{4FEE}', "xiu"),    // 修
    ('\u{5012}', "dao"),    // 倒
    ('\u{5019}', "hou"),    // 候
    ('\u{501A}', "yi"),     // 倚
    ('\u{501F}', "jie"),    // 借
    ('\u{503C}', "zhi"),    // 值
    ('\u{5047}', "jia"),    // 假
    ('\u{504F}', "pian"),   // 偏
    ('\u{505A}', "zuo"),    // 做
    ('\u{505C}', "ting"),   // 停
    ('\u{5065}', "jian"),   // 健
    ('\u{5076}', "ou"),     // 偶
    ('\u{507F}', "chang"),  // 偿
    ('\u{50CF}', "xiang"),  // 像
    ('\u{5112}', "ru"),     // 儒
    ('\u{513F}', "er"),     // 儿
    ('\u{5143}', "yuan"),   // 元
    ('\u{5144}', "xiong"),  // 兄
    ('\u{5145}', "chong"),  // 充
    ('\u{5148}', "xian"),   // 先
    ('\u{5149}', "guang"),  // 光
    ('\u{514B}', "ke"),     // 克
    ('\u{514D}', "mian"),   // 免
    ('\u{5151}', "dui"),    // 兑
    ('\u{5154}', "tu"),     // 兔
    ('\u{5165}', "ru"),     // 入
    ('\u{5168}', "quan"),   // 全
    ('\u{516B}', "ba"),     // 八
    ('\u{516D}', "liu"),    // 六
    ('\u{5170}', "lan"),    // 兰
    ('\u{5171}', "gong"),   // 共
    ('\u{5173}', "guan"),   // 关
    ('\u{5174}', "xing"),   // 兴
    ('\u{5175}', "bing"),   // 兵
    ('\u{5176}', "qi"),     // 其
    ('\u{5177}', "ju"),     // 具
    ('\u{5178}', "dian"),   // 典
    ('\u{517C}', "jian"),   // 兼
    ('\u{5185}', "nei"),    // 内
    ('\u{518D}', "zai"),    // 再
    ('\u{5199}', "xie"),    // 写
    ('\u{519B}', "jun"),    // 军
    ('\u{519C}', "nong"),   // 农
    ('\u{51B0}', "bing"),   // 冰
    ('\u{51B7}', "leng"),   // 冷
    ('\u{51CF}', "jian"),   // 减
    ('\u{51E0}', "ji"),     // 几
    ('\u{51E1}', "fan"),    // 凡
    ('\u{51FA}', "chu"),    // 出
    ('\u{51FB}', "ji"),     // 击
    ('\u{5206}', "fen"),    // 分
    ('\u{5207}', "qie"),    // 切
    ('\u{5208}', "yi"),     // 刈
    ('\u{5217}', "lie"),    // 列
    ('\u{521A}', "gang"),   // 刚
    ('\u{521B}', "chuang"), // 创
    ('\u{5220}', "shan"),   // 删
    ('\u{5224}', "pan"),    // 判
    ('\u{5229}', "li"),     // 利
    ('\u{522B}', "bie"),    // 别
    ('\u{5230}', "dao"),    // 到
    ('\u{5236}', "zhi"),    // 制
    ('\u{5237}', "shua"),   // 刷
    ('\u{523B}', "ke"),     // 刻
    ('\u{5242}', "ji"),     // 剂
    ('\u{524D}', "qian"),   // 前
    ('\u{526F}', "fu"),     // 副
    ('\u{529B}', "li"),     // 力
    ('\u{529E}', "ban"),    // 办
    ('\u{529F}', "gong"),   // 功
    ('\u{52A0}', "jia"),    // 加
    ('\u{52A1}', "wu"),     // 务
    ('\u{52A8}', "dong"),   // 动
    ('\u{52A9}', "zhu"),    // 助
    ('\u{52AA}', "nu"),     // 努
    ('\u{52B1}', "li"),     // 励
    ('\u{52BF}', "shi"),    // 势
    ('\u{52C7}', "yong"),   // 勇
    ('\u{52C9}', "mian"),   // 勉
    ('\u{5316}', "hua"),    // 化
    ('\u{5317}', "bei"),    // 北
    ('\u{533A}', "qu"),     // 区
    ('\u{5341}', "shi"),    // 十
    ('\u{5343}', "qian"),   // 千
    ('\u{5347}', "sheng"),  // 升
    ('\u{5348}', "wu"),     // 午
    ('\u{534A}', "ban"),    // 半
    ('\u{534E}', "hua"),    // 华
    ('\u{5355}', "dan"),    // 单
    ('\u{5357}', "nan"),    // 南
    ('\u{535A}', "bo"),     // 博
    ('\u{5360}', "zhan"),   // 占
    ('\u{5361}', "ka"),     // 卡
    ('\u{5370}', "yin"),    // 印
    ('\u{5371}', "wei"),    // 危
    ('\u{5373}', "ji"),     // 即
    ('\u{5374}', "que"),    // 却
    ('\u{5377}', "juan"),   // 卷
    ('\u{5386}', "li"),     // 历
    ('\u{5389}', "li"),     // 厉
    ('\u{538B}', "ya"),     // 压
    ('\u{5395}', "ce"),     // 厕
    ('\u{539F}', "yuan"),   // 原
    ('\u{53BB}', "qu"),     // 去
    ('\u{53C2}', "can"),    // 参
    ('\u{53C8}', "you"),    // 又
    ('\u{53CA}', "ji"),     // 及
    ('\u{53CB}', "you"),    // 友
    ('\u{53CC}', "shuang"), // 双
    ('\u{53CD}', "fan"),    // 反
    ('\u{53D1}', "fa"),     // 发
    ('\u{53D4}', "shu"),    // 叔
    ('\u{53D6}', "qu"),     // 取
    ('\u{53D7}', "shou"),   // 受
    ('\u{53D8}', "bian"),   // 变
    ('\u{53D9}', "xu"),     // 叙
    ('\u{53E3}', "kou"),    // 口
    ('\u{53E4}', "gu"),     // 古
    ('\u{53E5}', "ju"),     // 句
    ('\u{53E6}', "ling"),   // 另
    ('\u{53EA}', "zhi"),    // 只
    ('\u{53EB}', "jiao"),   // 叫
    ('\u{53EC}', "zhao"),   // 召
    ('\u{53EF}', "ke"),     // 可
    ('\u{53F0}', "tai"),    // 台
    ('\u{53F2}', "shi"),    // 史
    ('\u{53F3}', "you"),    // 右
    ('\u{53F7}', "hao"),    // 号
    ('\u{53F8}', "si"),     // 司
    ('\u{5403}', "chi"),    // 吃
    ('\u{540C}', "tong"),   // 同
    ('\u{540D}', "ming"),   // 名
    ('\u{540E}', "hou"),    // 后
    ('\u{5411}', "xiang"),  // 向
    ('\u{5417}', "ma"),     // 吗
    ('\u{5427}', "ba"),     // 吧
    ('\u{542C}', "ting"),   // 听
    ('\u{5439}', "chui"),   // 吹
    ('\u{5440}', "ya"),     // 呀
    ('\u{5442}', "lu"),     // 呂 — 吕
    ('\u{5457}', "bei"),    // 呗
    ('\u{545C}', "wu"),     // 呜
    ('\u{5473}', "wei"),    // 味
    ('\u{547C}', "hu"),     // 呼
    ('\u{547D}', "ming"),   // 命
    ('\u{548C}', "he"),     // 和
    ('\u{54C1}', "pin"),    // 品
    ('\u{54C8}', "ha"),     // 哈
    ('\u{54CD}', "xiang"),  // 响
    ('\u{54E5}', "ge"),     // 哥
    ('\u{54E6}', "o"),      // 哦
    ('\u{54EA}', "na"),     // 哪
    ('\u{54ED}', "ku"),     // 哭
    ('\u{54F2}', "zhe"),    // 哲
    ('\u{5510}', "tang"),   // 唐
    ('\u{552F}', "wei"),    // 唯
    ('\u{5531}', "chang"),  // 唱
    ('\u{5546}', "shang"),  // 商
    ('\u{554A}', "a"),      // 啊 — (dup guard, already above)
    ('\u{5566}', "la"),     // 啦
    ('\u{559C}', "xi"),     // 喜
    ('\u{559D}', "he"),     // 喝
    ('\u{561B}', "ma"),     // 嘛
    ('\u{5634}', "zui"),    // 嘴
    ('\u{56DB}', "si"),     // 四
    ('\u{56DE}', "hui"),    // 回
    ('\u{56E0}', "yin"),    // 因
    ('\u{56E2}', "tuan"),   // 团
    ('\u{56F4}', "wei"),    // 围
    ('\u{56FA}', "gu"),     // 固
    ('\u{56FD}', "guo"),    // 国
    ('\u{5708}', "quan"),   // 圈
    ('\u{571F}', "tu"),     // 土
    ('\u{5728}', "zai"),    // 在
    ('\u{5730}', "di"),     // 地
    ('\u{573A}', "chang"),  // 场
    ('\u{5747}', "jun"),    // 均
    ('\u{574A}', "fang"),   // 坊
    ('\u{5750}', "zuo"),    // 坐
    ('\u{5757}', "kuai"),   // 块
    ('\u{575A}', "jian"),   // 坚
    ('\u{575B}', "tan"),    // 坛
    ('\u{578B}', "xing"),   // 型
    ('\u{57CE}', "cheng"),  // 城
    ('\u{57DF}', "yu"),     // 域
    ('\u{57F9}', "pei"),    // 培
    ('\u{5802}', "tang"),   // 堂
    ('\u{5854}', "ta"),     // 塔
    ('\u{586B}', "tian"),   // 填
    ('\u{58EB}', "shi"),    // 士
    ('\u{58F0}', "sheng"),  // 声
    ('\u{5904}', "chu"),    // 处
    ('\u{5907}', "bei"),    // 备
    ('\u{590D}', "fu"),     // 复
    ('\u{590F}', "xia"),    // 夏
    ('\u{5916}', "wai"),    // 外
    ('\u{591A}', "duo"),    // 多
    ('\u{591C}', "ye"),     // 夜
    ('\u{5927}', "da"),     // 大
    ('\u{5929}', "tian"),   // 天
    ('\u{592A}', "tai"),    // 太
    ('\u{592B}', "fu"),     // 夫
    ('\u{5931}', "shi"),    // 失
    ('\u{5934}', "tou"),    // 头
    ('\u{5947}', "qi"),     // 奇
    ('\u{5949}', "feng"),   // 奉
    ('\u{5956}', "jiang"),  // 奖
    ('\u{5973}', "nu"),     // 女
    ('\u{5979}', "ta"),     // 她
    ('\u{597D}', "hao"),    // 好
    ('\u{5987}', "fu"),     // 妇
    ('\u{5988}', "ma"),     // 妈
    ('\u{5999}', "miao"),   // 妙
    ('\u{59B9}', "mei"),    // 妹
    ('\u{59BB}', "qi"),     // 妻
    ('\u{59CB}', "shi"),    // 始
    ('\u{59D0}', "jie"),    // 姐
    ('\u{59D1}', "gu"),     // 姑
    ('\u{59D3}', "xing"),   // 姓
    ('\u{59D4}', "wei"),    // 委
    ('\u{5A5A}', "hun"),    // 婚
    ('\u{5B50}', "zi"),     // 子
    ('\u{5B57}', "zi"),     // 字
    ('\u{5B58}', "cun"),    // 存
    ('\u{5B59}', "sun"),    // 孙
    ('\u{5B66}', "xue"),    // 学
    ('\u{5B69}', "hai"),    // 孩
    ('\u{5B78}', "xue"),    // 學 (Trad; kept for coverage)
    ('\u{5B83}', "ta"),     // 它
    ('\u{5B85}', "zhai"),   // 宅
    ('\u{5B87}', "yu"),     // 宇
    ('\u{5B88}', "shou"),   // 守
    ('\u{5B89}', "an"),     // 安
    ('\u{5B8C}', "wan"),    // 完
    ('\u{5B98}', "guan"),   // 官
    ('\u{5B9A}', "ding"),   // 定
    ('\u{5B9E}', "shi"),    // 实
    ('\u{5B9F}', "shi"),    // 実 — 實 (Trad)
    ('\u{5BA2}', "ke"),     // 客
    ('\u{5BA4}', "shi"),    // 室
    ('\u{5BB6}', "jia"),    // 家
    ('\u{5BB9}', "rong"),   // 容
    ('\u{5BBD}', "kuan"),   // 宽
    ('\u{5BBF}', "su"),     // 宿
    ('\u{5BC6}', "mi"),     // 密
    ('\u{5BCC}', "fu"),     // 富
    ('\u{5BF9}', "dui"),    // 对
    ('\u{5BFB}', "xun"),    // 寻
    ('\u{5BFC}', "dao"),    // 导
    ('\u{5C0F}', "xiao"),   // 小
    ('\u{5C11}', "shao"),   // 少
    ('\u{5C14}', "er"),     // 尔
    ('\u{5C1A}', "shang"),  // 尚
    ('\u{5C31}', "jiu"),    // 就
    ('\u{5C3A}', "chi"),    // 尺
    ('\u{5C3C}', "ni"),     // 尼
    ('\u{5C40}', "ju"),     // 局
    ('\u{5C42}', "ceng"),   // 层
    ('\u{5C45}', "ju"),     // 居
    ('\u{5C4B}', "wu"),     // 屋
    ('\u{5C55}', "zhan"),   // 展
    ('\u{5C5E}', "shu"),    // 属
    ('\u{5C71}', "shan"),   // 山
    ('\u{5CA9}', "yan"),    // 岩
    ('\u{5CB8}', "an"),     // 岸
    ('\u{5CF6}', "dao"),    // 島 (Trad)
    ('\u{5DDD}', "chuan"),  // 川
    ('\u{5DE5}', "gong"),   // 工
    ('\u{5DE6}', "zuo"),    // 左
    ('\u{5DE7}', "qiao"),   // 巧
    ('\u{5DEE}', "cha"),    // 差
    ('\u{5DF1}', "ji"),     // 己
    ('\u{5DF2}', "yi"),     // 已
    ('\u{5DF4}', "ba"),     // 巴
    ('\u{5E02}', "shi"),    // 市
    ('\u{5E03}', "bu"),     // 布
    ('\u{5E08}', "shi"),    // 师
    ('\u{5E0C}', "xi"),     // 希
    ('\u{5E10}', "zhang"),  // 帐
    ('\u{5E15}', "pa"),     // 帕
    ('\u{5E26}', "dai"),    // 带
    ('\u{5E2D}', "xi"),     // 席
    ('\u{5E2E}', "bang"),   // 帮
    ('\u{5E38}', "chang"),  // 常
    ('\u{5E45}', "fu"),     // 幅
    ('\u{5E73}', "ping"),   // 平
    ('\u{5E74}', "nian"),   // 年
    ('\u{5E76}', "bing"),   // 并
    ('\u{5E78}', "xing"),   // 幸
    ('\u{5E7C}', "you"),    // 幼
    ('\u{5E7F}', "guang"),  // 广
    ('\u{5E84}', "zhuang"), // 庄
    ('\u{5E86}', "qing"),   // 庆
    ('\u{5E8A}', "chuang"), // 床
    ('\u{5E8F}', "xu"),     // 序
    ('\u{5E93}', "ku"),     // 库
    ('\u{5E94}', "ying"),   // 应
    ('\u{5E95}', "di"),     // 底
    ('\u{5E97}', "dian"),   // 店
    ('\u{5EA6}', "du"),     // 度
    ('\u{5EA7}', "zuo"),    // 座
    ('\u{5EAD}', "ting"),   // 庭
    ('\u{5EB7}', "kang"),   // 康
    ('\u{5EFA}', "jian"),   // 建
    ('\u{5F00}', "kai"),    // 开
    ('\u{5F02}', "yi"),     // 异
    ('\u{5F03}', "qi"),     // 弃
    ('\u{5F0A}', "bi"),     // 弊
    ('\u{5F0F}', "shi"),    // 式
    ('\u{5F15}', "yin"),    // 引
    ('\u{5F1F}', "di"),     // 弟
    ('\u{5F20}', "zhang"),  // 张
    ('\u{5F25}', "mi"),     // 弥
    ('\u{5F31}', "ruo"),    // 弱
    ('\u{5F3A}', "qiang"),  // 强
    ('\u{5F52}', "gui"),    // 归
    ('\u{5F53}', "dang"),   // 当
    ('\u{5F55}', "lu"),     // 录
    ('\u{5F62}', "xing"),   // 形
    ('\u{5F69}', "cai"),    // 彩
    ('\u{5F71}', "ying"),   // 影
    ('\u{5F79}', "yi"),     // 役
    ('\u{5F7B}', "che"),    // 彻
    ('\u{5F7C}', "bi"),     // 彼
    ('\u{5F80}', "wang"),   // 往
    ('\u{5F81}', "zheng"),  // 征
    ('\u{5F84}', "jing"),   // 径
    ('\u{5F85}', "dai"),    // 待
    ('\u{5F88}', "hen"),    // 很
    ('\u{5F8B}', "lu"),     // 律
    ('\u{5F8C}', "hou"),    // 後 (Trad)
    ('\u{5F97}', "de"),     // 得
    ('\u{5FAA}', "xun"),    // 循
    ('\u{5FAE}', "wei"),    // 微
    ('\u{5FC3}', "xin"),    // 心
    ('\u{5FC5}', "bi"),     // 必
    ('\u{5FCD}', "ren"),    // 忍
    ('\u{5FD7}', "zhi"),    // 志
    ('\u{5FD8}', "wang"),   // 忘
    ('\u{5FD9}', "mang"),   // 忙
    ('\u{5FEB}', "kuai"),   // 快
    ('\u{5FF5}', "nian"),   // 念
    ('\u{6001}', "tai"),    // 态
    ('\u{6015}', "pa"),     // 怕
    ('\u{601D}', "si"),     // 思
    ('\u{6027}', "xing"),   // 性
    ('\u{6028}', "yuan"),   // 怨
    ('\u{602A}', "guai"),   // 怪
    ('\u{6050}', "kong"),   // 恐
    ('\u{6068}', "hen"),    // 恨
    ('\u{6069}', "en"),     // 恩
    ('\u{6070}', "qia"),    // 恰
    ('\u{6076}', "e"),      // 恶
    ('\u{6084}', "qiao"),   // 悄
    ('\u{60A6}', "yue"),    // 悦
    ('\u{60A8}', "nin"),    // 您
    ('\u{60B2}', "bei"),    // 悲
    ('\u{60C5}', "qing"),   // 情
    ('\u{60CA}', "jing"),   // 惊
    ('\u{60D1}', "huo"),    // 惑
    ('\u{60E0}', "hui"),    // 惠
    ('\u{60E7}', "ju"),     // 惧
    ('\u{60E8}', "can"),    // 惨
    ('\u{60F3}', "xiang"),  // 想
    ('\u{60F9}', "re"),     // 惹
    ('\u{6108}', "yu"),     // 愈
    ('\u{610F}', "yi"),     // 意
    ('\u{611F}', "gan"),    // 感
    ('\u{6127}', "kui"),    // 愧
    ('\u{6148}', "ci"),     // 慈
    ('\u{6155}', "mu"),     // 慕
    ('\u{6162}', "man"),    // 慢
    ('\u{6167}', "hui"),    // 慧
    ('\u{6170}', "wei"),    // 慰
    ('\u{6210}', "cheng"),  // 成
    ('\u{6211}', "wo"),     // 我
    ('\u{6216}', "huo"),    // 或
    ('\u{6218}', "zhan"),   // 战
    ('\u{6234}', "dai"),    // 戴
    ('\u{6237}', "hu"),     // 户
    ('\u{6240}', "suo"),    // 所
    ('\u{624B}', "shou"),   // 手
    ('\u{624D}', "cai"),    // 才
    ('\u{6253}', "da"),     // 打
    ('\u{6254}', "reng"),   // 扔
    ('\u{6258}', "tuo"),    // 托
    ('\u{6263}', "kou"),    // 扣
    ('\u{6267}', "zhi"),    // 执
    ('\u{6269}', "kuo"),    // 扩
    ('\u{626B}', "sao"),    // 扫
    ('\u{626C}', "yang"),   // 扬
    ('\u{627E}', "zhao"),   // 找
    ('\u{627F}', "cheng"),  // 承
    ('\u{6280}', "ji"),     // 技
    ('\u{6284}', "chao"),   // 抄
    ('\u{6291}', "yi"),     // 抑
    ('\u{6293}', "zhua"),   // 抓
    ('\u{6295}', "tou"),    // 投
    ('\u{6297}', "kang"),   // 抗
    ('\u{6298}', "zhe"),    // 折
    ('\u{62A2}', "qiang"),  // 抢
    ('\u{62A5}', "bao"),    // 报
    ('\u{62B5}', "di"),     // 抵
    ('\u{62BD}', "chou"),   // 抽
    ('\u{62C5}', "dan"),    // 担
    ('\u{62C6}', "chai"),   // 拆
    ('\u{62C9}', "la"),     // 拉
    ('\u{62CD}', "pai"),    // 拍
    ('\u{62DB}', "zhao"),   // 招
    ('\u{62E5}', "yong"),   // 拥
    ('\u{62E9}', "ze"),     // 择
    ('\u{62EC}', "kuo"),    // 括
    ('\u{62ED}', "shi"),    // 拭
    ('\u{62FF}', "na"),     // 拿
    ('\u{6301}', "chi"),    // 持
    ('\u{6307}', "zhi"),    // 指
    ('\u{6309}', "an"),     // 按
    ('\u{6321}', "dang"),   // 挡
    ('\u{6323}', "zheng"),  // 挣
    ('\u{632F}', "zhen"),   // 振
    ('\u{6355}', "bu"),     // 捕
    ('\u{6362}', "huan"),   // 换
    ('\u{636E}', "ju"),     // 据
    ('\u{6377}', "jie"),    // 捷
    ('\u{6388}', "shou"),   // 授
    ('\u{6389}', "diao"),   // 掉
    ('\u{6392}', "pai"),    // 排
    ('\u{63A2}', "tan"),    // 探
    ('\u{63A5}', "jie"),    // 接
    ('\u{63A7}', "kong"),   // 控
    ('\u{63A8}', "tui"),    // 推
    ('\u{63AA}', "cuo"),    // 措
    ('\u{63CF}', "miao"),   // 描
    ('\u{63D0}', "ti"),     // 提
    ('\u{63D2}', "cha"),    // 插
    ('\u{63E1}', "wo"),     // 握
    ('\u{641C}', "sou"),    // 搜
    ('\u{642C}', "ban"),    // 搬
    ('\u{6444}', "she"),    // 摄
    ('\u{6446}', "bai"),    // 摆
    ('\u{6458}', "zhai"),   // 摘
    ('\u{6479}', "mo"),     // 摹
    ('\u{6491}', "cheng"),  // 撑
    ('\u{64AD}', "bo"),     // 播
    ('\u{64B0}', "zhuan"),  // 撰
    ('\u{64CD}', "cao"),    // 操
    ('\u{6539}', "gai"),    // 改
    ('\u{653B}', "gong"),   // 攻
    ('\u{653E}', "fang"),   // 放
    ('\u{653F}', "zheng"),  // 政
    ('\u{6545}', "gu"),     // 故
    ('\u{6548}', "xiao"),   // 效
    ('\u{654C}', "di"),     // 敌
    ('\u{6551}', "jiu"),    // 救
    ('\u{6559}', "jiao"),   // 教
    ('\u{6562}', "gan"),    // 敢
    ('\u{6563}', "san"),    // 散
    ('\u{6570}', "shu"),    // 数
    ('\u{6574}', "zheng"),  // 整
    ('\u{6587}', "wen"),    // 文
    ('\u{6591}', "ban"),    // 斑
    ('\u{6597}', "dou"),    // 斗
    ('\u{659C}', "xie"),    // 斜
    ('\u{65AD}', "duan"),   // 断
    ('\u{65B0}', "xin"),    // 新
    ('\u{65B9}', "fang"),   // 方
    ('\u{65BD}', "shi"),    // 施
    ('\u{65C5}', "lu"),     // 旅
    ('\u{65CF}', "zu"),     // 族
    ('\u{65D7}', "qi"),     // 旗
    ('\u{65E0}', "wu"),     // 无
    ('\u{65E2}', "ji"),     // 既
    ('\u{65E5}', "ri"),     // 日
    ('\u{65E7}', "jiu"),    // 旧
    ('\u{65E9}', "zao"),    // 早
    ('\u{65EC}', "xun"),    // 旬
    ('\u{65F6}', "shi"),    // 时
    ('\u{65FA}', "wang"),   // 旺
    ('\u{660E}', "ming"),   // 明
    ('\u{660F}', "hun"),    // 昏
    ('\u{6613}', "yi"),     // 易
    ('\u{6614}', "xi"),     // 昔
    ('\u{661F}', "xing"),   // 星
    ('\u{6620}', "ying"),   // 映
    ('\u{6625}', "chun"),   // 春
    ('\u{6628}', "zuo"),    // 昨
    ('\u{662D}', "zhao"),   // 昭
    ('\u{662F}', "shi"),    // 是
    ('\u{663E}', "xian"),   // 显
    ('\u{664B}', "jin"),    // 晋
    ('\u{665A}', "wan"),    // 晚
    ('\u{6668}', "chen"),   // 晨
    ('\u{666E}', "pu"),     // 普
    ('\u{666F}', "jing"),   // 景
    ('\u{6674}', "qing"),   // 晴
    ('\u{6676}', "jing"),   // 晶
    ('\u{667A}', "zhi"),    // 智
    ('\u{6682}', "zan"),    // 暂
    ('\u{6697}', "an"),     // 暗
    ('\u{66B4}', "bao"),    // 暴
    ('\u{66F2}', "qu"),     // 曲
    ('\u{66F4}', "geng"),   // 更
    ('\u{66FE}', "zeng"),   // 曾
    ('\u{66FF}', "ti"),     // 替
    ('\u{6700}', "zui"),    // 最
    ('\u{6708}', "yue"),    // 月
    ('\u{6709}', "you"),    // 有
    ('\u{670B}', "peng"),   // 朋
    ('\u{670D}', "fu"),     // 服
    ('\u{671B}', "wang"),   // 望
    ('\u{671D}', "chao"),   // 朝
    ('\u{671F}', "qi"),     // 期
    ('\u{6728}', "mu"),     // 木
    ('\u{672A}', "wei"),    // 未
    ('\u{672B}', "mo"),     // 末
    ('\u{672C}', "ben"),    // 本
    ('\u{672F}', "shu"),    // 术
    ('\u{6731}', "zhu"),    // 朱
    ('\u{6735}', "duo"),    // 朵
    ('\u{6742}', "za"),     // 杂
    ('\u{6743}', "quan"),   // 权
    ('\u{6746}', "gan"),    // 杆
    ('\u{674E}', "li"),     // 李
    ('\u{6750}', "cai"),    // 材
    ('\u{6751}', "cun"),    // 村
    ('\u{675F}', "shu"),    // 束
    ('\u{6761}', "tiao"),   // 条
    ('\u{6765}', "lai"),    // 来
    ('\u{6768}', "yang"),   // 杨
    ('\u{677E}', "song"),   // 松
    ('\u{677F}', "ban"),    // 板
    ('\u{6781}', "ji"),     // 极
    ('\u{6784}', "gou"),    // 构
    ('\u{6790}', "xi"),     // 析
    ('\u{6797}', "lin"),    // 林
    ('\u{679C}', "guo"),    // 果
    ('\u{67B6}', "jia"),    // 架
    ('\u{67D0}', "mou"),    // 某
    ('\u{67D3}', "ran"),    // 染
    ('\u{67D4}', "rou"),    // 柔
    ('\u{67E5}', "cha"),    // 查
    ('\u{6807}', "biao"),   // 标
    ('\u{6811}', "shu"),    // 树
    ('\u{6821}', "xiao"),   // 校
    ('\u{6837}', "yang"),   // 样
    ('\u{6838}', "he"),     // 核
    ('\u{6839}', "gen"),    // 根
    ('\u{683C}', "ge"),     // 格
    ('\u{6842}', "gui"),    // 桂
    ('\u{6843}', "tao"),    // 桃
    ('\u{6848}', "an"),     // 案
    ('\u{6851}', "sang"),   // 桑
    ('\u{6865}', "qiao"),   // 桥
    ('\u{68A6}', "meng"),   // 梦
    ('\u{68C0}', "jian"),   // 检
    ('\u{68EE}', "sen"),    // 森
    ('\u{6905}', "yi"),     // 椅
    ('\u{690D}', "zhi"),    // 植
    ('\u{6A21}', "mo"),     // 模
    ('\u{6A29}', "quan"),   // 権 — 權 (Trad)
    ('\u{6A2A}', "heng"),   // 横
    ('\u{6A5F}', "ji"),     // 機 (Trad)
    ('\u{6B21}', "ci"),     // 次
    ('\u{6B22}', "huan"),   // 欢
    ('\u{6B27}', "ou"),     // 欧
    ('\u{6B32}', "yu"),     // 欲
    ('\u{6B4C}', "ge"),     // 歌
    ('\u{6B62}', "zhi"),    // 止
    ('\u{6B63}', "zheng"),  // 正
    ('\u{6B64}', "ci"),     // 此
    ('\u{6B65}', "bu"),     // 步
    ('\u{6B66}', "wu"),     // 武
    ('\u{6B67}', "qi"),     // 歧
    ('\u{6B7B}', "si"),     // 死
    ('\u{6B8A}', "shu"),    // 殊
    ('\u{6BB5}', "duan"),   // 段
    ('\u{6BCD}', "mu"),     // 母
    ('\u{6BCF}', "mei"),    // 每
    ('\u{6BD4}', "bi"),     // 比
    ('\u{6BDB}', "mao"),    // 毛
    ('\u{6C0F}', "shi"),    // 氏
    ('\u{6C11}', "min"),    // 民
    ('\u{6C14}', "qi"),     // 气
    ('\u{6C34}', "shui"),   // 水
    ('\u{6C38}', "yong"),   // 永
    ('\u{6C42}', "qiu"),    // 求
    ('\u{6C49}', "han"),    // 汉
    ('\u{6C57}', "han"),    // 汗
    ('\u{6C5F}', "jiang"),  // 江
    ('\u{6C60}', "chi"),    // 池
    ('\u{6C61}', "wu"),     // 污
    ('\u{6C64}', "tang"),   // 汤
    ('\u{6C7A}', "jue"),    // 決 (Trad)
    ('\u{6C88}', "chen"),   // 沈
    ('\u{6CA1}', "mei"),    // 没
    ('\u{6CB3}', "he"),     // 河
    ('\u{6CB9}', "you"),    // 油
    ('\u{6CBB}', "zhi"),    // 治
    ('\u{6CC9}', "quan"),   // 泉
    ('\u{6CD5}', "fa"),     // 法
    ('\u{6CE2}', "bo"),     // 波
    ('\u{6CE5}', "ni"),     // 泥
    ('\u{6CE8}', "zhu"),    // 注
    ('\u{6CFC}', "po"),     // 泼
    ('\u{6D0B}', "yang"),   // 洋
    ('\u{6D17}', "xi"),     // 洗
    ('\u{6D1E}', "dong"),   // 洞
    ('\u{6D25}', "jin"),    // 津
    ('\u{6D3B}', "huo"),    // 活
    ('\u{6D3D}', "qia"),    // 洽
    ('\u{6D3E}', "pai"),    // 派
    ('\u{6D41}', "liu"),    // 流
    ('\u{6D4B}', "ce"),     // 测
    ('\u{6D4E}', "ji"),     // 济
    ('\u{6D53}', "nong"),   // 浓
    ('\u{6D59}', "zhe"),    // 浙
    ('\u{6D77}', "hai"),    // 海
    ('\u{6D82}', "tu"),     // 涂
    ('\u{6D88}', "xiao"),   // 消
    ('\u{6D89}', "she"),    // 涉
    ('\u{6DB2}', "ye"),     // 液
    ('\u{6DE1}', "dan"),    // 淡
    ('\u{6DF1}', "shen"),   // 深
    ('\u{6E05}', "qing"),   // 清
    ('\u{6E10}', "jian"),   // 渐
    ('\u{6E29}', "wen"),    // 温
    ('\u{6E2F}', "gang"),   // 港
    ('\u{6E38}', "you"),    // 游
    ('\u{6E67}', "yong"),   // 湧 — 涌
    ('\u{6E80}', "man"),    // 満 (Trad variant)
    ('\u{6EE1}', "man"),    // 满
    ('\u{6EE4}', "lu"),     // 滤
    ('\u{6EF4}', "di"),     // 滴
    ('\u{6F02}', "piao"),   // 漂
    ('\u{6F14}', "yan"),    // 演
    ('\u{6F5C}', "qian"),   // 潜
    ('\u{6FC0}', "ji"),     // 激
    ('\u{706B}', "huo"),    // 火
    ('\u{706F}', "deng"),   // 灯
    ('\u{7075}', "ling"),   // 灵
    ('\u{707E}', "zai"),    // 灾
    ('\u{70AD}', "tan"),    // 炭
    ('\u{70B9}', "dian"),   // 点
    ('\u{70BC}', "lian"),   // 炼
    ('\u{70C8}', "lie"),    // 烈
    ('\u{70DF}', "yan"),    // 烟
    ('\u{7136}', "ran"),    // 然
    ('\u{7167}', "zhao"),   // 照
    ('\u{7231}', "ai"),     // 爱
    ('\u{7236}', "fu"),     // 父
    ('\u{7238}', "ba"),     // 爸
    ('\u{7247}', "pian"),   // 片
    ('\u{724C}', "pai"),    // 牌
    ('\u{7259}', "ya"),     // 牙
    ('\u{725B}', "niu"),    // 牛
    ('\u{7269}', "wu"),     // 物
    ('\u{72AC}', "quan"),   // 犬
    ('\u{72B6}', "zhuang"), // 状
    ('\u{72EC}', "du"),     // 独
    ('\u{732E}', "xian"),   // 献
    ('\u{738B}', "wang"),   // 王
    ('\u{73B0}', "xian"),   // 现
    ('\u{7406}', "li"),     // 理
    ('\u{751F}', "sheng"),  // 生
    ('\u{7528}', "yong"),   // 用
    ('\u{7530}', "tian"),   // 田
    ('\u{7531}', "you"),    // 由
    ('\u{7535}', "dian"),   // 电
    ('\u{7537}', "nan"),    // 男
    ('\u{754C}', "jie"),    // 界
    ('\u{7559}', "liu"),    // 留
    ('\u{756A}', "fan"),    // 番
    ('\u{7591}', "yi"),     // 疑
    ('\u{767D}', "bai"),    // 白
    ('\u{767E}', "bai"),    // 百
    ('\u{7684}', "de"),     // 的
    ('\u{76AE}', "pi"),     // 皮
    ('\u{76D6}', "gai"),    // 盖
    ('\u{76DB}', "sheng"),  // 盛
    ('\u{76EE}', "mu"),     // 目
    ('\u{76F4}', "zhi"),    // 直
    ('\u{76F8}', "xiang"),  // 相
    ('\u{770B}', "kan"),    // 看
    ('\u{771F}', "zhen"),   // 真
    ('\u{7720}', "mian"),   // 眠
    ('\u{773C}', "yan"),    // 眼
    ('\u{7761}', "shui"),   // 睡
    ('\u{77E5}', "zhi"),    // 知
    ('\u{77ED}', "duan"),   // 短
    ('\u{7801}', "ma"),     // 码
    ('\u{7814}', "yan"),    // 研
    ('\u{7834}', "po"),     // 破
    ('\u{786C}', "ying"),   // 硬
    ('\u{78C1}', "ci"),     // 磁
    ('\u{793A}', "shi"),    // 示
    ('\u{793E}', "she"),    // 社
    ('\u{795E}', "shen"),   // 神
    ('\u{7968}', "piao"),   // 票
    ('\u{79BB}', "li"),     // 离
    ('\u{79C1}', "si"),     // 私
    ('\u{79CB}', "qiu"),    // 秋
    ('\u{79D1}', "ke"),     // 科
    ('\u{79FB}', "yi"),     // 移
    ('\u{7A0B}', "cheng"),  // 程
    ('\u{7A0D}', "shao"),   // 稍
    ('\u{7A33}', "wen"),    // 稳
    ('\u{7A46}', "mu"),     // 穆
    ('\u{7A76}', "jiu"),    // 究 (dup guard, already above)
    ('\u{7A7A}', "kong"),   // 空
    ('\u{7ACB}', "li"),     // 立
    ('\u{7ADE}', "jing"),   // 竞
    ('\u{7AE0}', "zhang"),  // 章
    ('\u{7B11}', "xiao"),   // 笑
    ('\u{7B14}', "bi"),     // 笔
    ('\u{7B2C}', "di"),     // 第
    ('\u{7B49}', "deng"),   // 等
    ('\u{7B54}', "da"),     // 答
    ('\u{7B56}', "ce"),     // 策
    ('\u{7B80}', "jian"),   // 简
    ('\u{7B97}', "suan"),   // 算
    ('\u{7BC7}', "pian"),   // 篇
    ('\u{7C7B}', "lei"),    // 类
    ('\u{7CBE}', "jing"),   // 精
    ('\u{7CDF}', "zao"),    // 糟
    ('\u{7CFB}', "xi"),     // 系
    ('\u{7EA2}', "hong"),   // 红
    ('\u{7EA6}', "yue"),    // 约
    ('\u{7EA7}', "ji"),     // 级
    ('\u{7EAA}', "ji"),     // 纪
    ('\u{7EBA}', "fang"),   // 纺
    ('\u{7EBF}', "xian"),   // 线
    ('\u{7EC4}', "zu"),     // 组
    ('\u{7EC6}', "xi"),     // 细
    ('\u{7EC8}', "zhong"),  // 终
    ('\u{7ECF}', "jing"),   // 经
    ('\u{7ED3}', "jie"),    // 结
    ('\u{7ED9}', "gei"),    // 给
    ('\u{7EDD}', "jue"),    // 绝
    ('\u{7EDF}', "tong"),   // 统
    ('\u{7EE7}', "ji"),     // 继
    ('\u{7EED}', "xu"),     // 续
    ('\u{7EF4}', "wei"),    // 维
    ('\u{7EFF}', "lu"),     // 绿
    ('\u{7F16}', "bian"),   // 编
    ('\u{7F51}', "wang"),   // 网
    ('\u{7F6E}', "zhi"),    // 置
    ('\u{7F8E}', "mei"),    // 美
    ('\u{7FA4}', "qun"),    // 群
    ('\u{8001}', "lao"),    // 老
    ('\u{800C}', "er"),     // 而
    ('\u{80A1}', "gu"),     // 股
    ('\u{80B2}', "yu"),     // 育
    ('\u{80CC}', "bei"),    // 背
    ('\u{80DE}', "bao"),    // 胞
    ('\u{80FD}', "neng"),   // 能
    ('\u{8106}', "cui"),    // 脆
    ('\u{8111}', "nao"),    // 脑
    ('\u{811A}', "jiao"),   // 脚
    ('\u{8138}', "lian"),   // 脸
    ('\u{8154}', "qiang"),  // 腔
    ('\u{81EA}', "zi"),     // 自
    ('\u{81F3}', "zhi"),    // 至
    ('\u{81F4}', "zhi"),    // 致
    ('\u{8206}', "yu"),     // 舆
    ('\u{820D}', "she"),    // 舍
    ('\u{821F}', "zhou"),   // 舟
    ('\u{822A}', "hang"),   // 航
    ('\u{822B}', "fang"),   // 舫
    ('\u{8239}', "chuan"),  // 船
    ('\u{8272}', "se"),     // 色
    ('\u{827A}', "yi"),     // 艺
    ('\u{8282}', "jie"),    // 节
    ('\u{82B1}', "hua"),    // 花
    ('\u{82B3}', "fang"),   // 芳
    ('\u{82CF}', "su"),     // 苏
    ('\u{82E5}', "ruo"),    // 若
    ('\u{82E6}', "ku"),     // 苦
    ('\u{82F1}', "ying"),   // 英
    ('\u{830E}', "jing"),   // 茎
    ('\u{831C}', "qian"),   // 茜
    ('\u{8336}', "cha"),    // 茶
    ('\u{8377}', "he"),     // 荷
    ('\u{8389}', "li"),     // 莉
    ('\u{83AB}', "mo"),     // 莫
    ('\u{83B7}', "huo"),    // 获
    ('\u{83DC}', "cai"),    // 菜
    ('\u{843D}', "luo"),    // 落
    ('\u{84DD}', "lan"),    // 蓝
    ('\u{865A}', "xu"),     // 虚
    ('\u{866B}', "chong"),  // 虫
    ('\u{884C}', "xing"),   // 行
    ('\u{8863}', "yi"),     // 衣
    ('\u{8868}', "biao"),   // 表
    ('\u{886B}', "shan"),   // 衫
    ('\u{8896}', "xiu"),    // 袖
    ('\u{88AB}', "bei"),    // 被
    ('\u{897F}', "xi"),     // 西
    ('\u{8981}', "yao"),    // 要
    ('\u{89C1}', "jian"),   // 见
    ('\u{89C2}', "guan"),   // 观
    ('\u{89C4}', "gui"),    // 规
    ('\u{89C6}', "shi"),    // 视
    ('\u{89D2}', "jiao"),   // 角
    ('\u{89E3}', "jie"),    // 解
    ('\u{8A00}', "yan"),    // 言
    ('\u{8B49}', "zheng"),  // 證 (Trad)
    ('\u{8BA1}', "ji"),     // 计
    ('\u{8BA4}', "ren"),    // 认
    ('\u{8BA9}', "rang"),   // 让
    ('\u{8BAD}', "xun"),    // 训
    ('\u{8BAE}', "yi"),     // 议
    ('\u{8BB0}', "ji"),     // 记
    ('\u{8BB2}', "jiang"),  // 讲
    ('\u{8BB8}', "xu"),     // 许
    ('\u{8BBA}', "lun"),    // 论
    ('\u{8BBE}', "she"),    // 设
    ('\u{8BBF}', "fang"),   // 访
    ('\u{8BC1}', "zheng"),  // 证
    ('\u{8BC4}', "ping"),   // 评
    ('\u{8BC6}', "shi"),    // 识
    ('\u{8BC9}', "su"),     // 诉
    ('\u{8BCD}', "ci"),     // 词
    ('\u{8BD5}', "shi"),    // 试
    ('\u{8BD7}', "shi"),    // 诗
    ('\u{8BDD}', "hua"),    // 话
    ('\u{8BE2}', "xun"),    // 询
    ('\u{8BE5}', "gai"),    // 该
    ('\u{8BE6}', "xiang"),  // 详
    ('\u{8BED}', "yu"),     // 语
    ('\u{8BF4}', "shuo"),   // 说
    ('\u{8BF7}', "qing"),   // 请
    ('\u{8BFB}', "du"),     // 读
    ('\u{8BFE}', "ke"),     // 课
    ('\u{8C08}', "tan"),    // 谈
    ('\u{8C0B}', "mou"),    // 谋
    ('\u{8C22}', "xie"),    // 谢
    ('\u{8C28}', "jin"),    // 谨
    ('\u{8C37}', "gu"),     // 谷
    ('\u{8C46}', "dou"),    // 豆
    ('\u{8C61}', "xiang"),  // 象
    ('\u{8D1F}', "fu"),     // 负
    ('\u{8D22}', "cai"),    // 财
    ('\u{8D23}', "ze"),     // 责
    ('\u{8D28}', "zhi"),    // 质
    ('\u{8D29}', "fan"),    // 贩
    ('\u{8D2D}', "gou"),    // 购
    ('\u{8D2F}', "guan"),   // 贯
    ('\u{8D39}', "fei"),    // 费
    ('\u{8D3A}', "he"),     // 贺
    ('\u{8D3E}', "jia"),    // 贾
    ('\u{8D44}', "zi"),     // 资
    ('\u{8D4A}', "she"),    // 赊
    ('\u{8D64}', "chi"),    // 赤
    ('\u{8D70}', "zou"),    // 走
    ('\u{8D77}', "qi"),     // 起
    ('\u{8D85}', "chao"),   // 超
    ('\u{8D8A}', "yue"),    // 越
    ('\u{8DB3}', "zu"),     // 足
    ('\u{8DD1}', "pao"),    // 跑
    ('\u{8DDD}', "ju"),     // 距
    ('\u{8DEF}', "lu"),     // 路
    ('\u{8DF3}', "tiao"),   // 跳
    ('\u{8E22}', "ti"),     // 踢
    ('\u{8E29}', "cai"),    // 踩
    ('\u{8EAB}', "shen"),   // 身
    ('\u{8F66}', "che"),    // 车
    ('\u{8F6C}', "zhuan"),  // 转
    ('\u{8F6E}', "lun"),    // 轮
    ('\u{8F7B}', "qing"),   // 轻
    ('\u{8F83}', "jiao"),   // 较
    ('\u{8F89}', "hui"),    // 辉
    ('\u{8F91}', "ji"),     // 辑
    ('\u{8F93}', "shu"),    // 输
    ('\u{8FA9}', "bian"),   // 辩
    ('\u{8FB9}', "bian"),   // 边
    ('\u{8FBE}', "da"),     // 达
    ('\u{8FC1}', "qian"),   // 迁
    ('\u{8FC5}', "xun"),    // 迅
    ('\u{8FD1}', "jin"),    // 近
    ('\u{8FD4}', "fan"),    // 返
    ('\u{8FD8}', "hai"),    // 还
    ('\u{8FD9}', "zhe"),    // 这
    ('\u{8FDB}', "jin"),    // 进
    ('\u{8FDC}', "yuan"),   // 远
    ('\u{8FDE}', "lian"),   // 连
    ('\u{8FDF}', "chi"),    // 迟
    ('\u{8FEB}', "po"),     // 迫
    ('\u{9002}', "shi"),    // 适
    ('\u{9009}', "xuan"),   // 选
    ('\u{900F}', "tou"),    // 透
    ('\u{9010}', "zhu"),    // 逐
    ('\u{9014}', "tu"),     // 途
    ('\u{901A}', "tong"),   // 通
    ('\u{901F}', "su"),     // 速
    ('\u{9020}', "zao"),    // 造
    ('\u{9053}', "dao"),    // 道
    ('\u{9057}', "yi"),     // 遗
    ('\u{9075}', "zun"),    // 遵
    ('\u{90A3}', "na"),     // 那
    ('\u{90AE}', "you"),    // 邮
    ('\u{90BB}', "lin"),    // 邻
    ('\u{90E1}', "jun"),    // 郡
    ('\u{90E8}', "bu"),     // 部
    ('\u{91CD}', "zhong"),  // 重
    ('\u{91CE}', "ye"),     // 野
    ('\u{91CF}', "liang"),  // 量
    ('\u{91D1}', "jin"),    // 金
    ('\u{9274}', "jian"),   // 鉴
    ('\u{94F6}', "yin"),    // 银
    ('\u{9500}', "xiao"),   // 销
    ('\u{957F}', "chang"),  // 长
    ('\u{95E8}', "men"),    // 门
    ('\u{95EE}', "wen"),    // 问
    ('\u{95F4}', "jian"),   // 间
    ('\u{95F7}', "men"),    // 闷
    ('\u{9605}', "yue"),    // 阅
    ('\u{9633}', "yang"),   // 阳
    ('\u{9634}', "yin"),    // 阴
    ('\u{9636}', "jie"),    // 阶
    ('\u{9645}', "ji"),     // 际
    ('\u{9648}', "chen"),   // 陈
    ('\u{9650}', "xian"),   // 限
    ('\u{9655}', "shan"),   // 陕
    ('\u{9662}', "yuan"),   // 院
    ('\u{9664}', "chu"),    // 除
    ('\u{9668}', "yun"),    // 陨
    ('\u{9669}', "xian"),   // 险
    ('\u{9673}', "chen"),   // 陳 (Trad)
    ('\u{968F}', "sui"),    // 随
    ('\u{9690}', "yin"),    // 隐
    ('\u{9694}', "ge"),     // 隔
    ('\u{9698}', "ai"),     // 隘
    ('\u{9699}', "xi"),     // 隙
    ('\u{96C4}', "xiong"),  // 雄
    ('\u{96C6}', "ji"),     // 集
    ('\u{96CD}', "yong"),   // 雍
    ('\u{96E8}', "yu"),     // 雨
    ('\u{96EA}', "xue"),    // 雪
    ('\u{96F6}', "ling"),   // 零
    ('\u{96F7}', "lei"),    // 雷
    ('\u{96FB}', "dian"),   // 電 (Trad)
    ('\u{9700}', "xu"),     // 需
    ('\u{9752}', "qing"),   // 青
    ('\u{975E}', "fei"),    // 非
    ('\u{9762}', "mian"),   // 面
    ('\u{97F3}', "yin"),    // 音
    ('\u{9875}', "ye"),     // 页
    ('\u{9879}', "xiang"),  // 项
    ('\u{987A}', "shun"),   // 顺
    ('\u{987B}', "xu"),     // 须
    ('\u{9884}', "yu"),     // 预
    ('\u{9886}', "ling"),   // 领
    ('\u{9891}', "pin"),    // 频
    ('\u{98CE}', "feng"),   // 风
    ('\u{98DE}', "fei"),    // 飞
    ('\u{98DF}', "shi"),    // 食
    ('\u{9910}', "can"),    // 餐
    ('\u{9996}', "shou"),   // 首
    ('\u{9999}', "xiang"),  // 香
    ('\u{9A6C}', "ma"),     // 马
    ('\u{9A76}', "shi"),    // 驶
    ('\u{9A8C}', "yan"),    // 验
    ('\u{9AD8}', "gao"),    // 高
    ('\u{9CE5}', "niao"),   // 鳥 — 鸟
    ('\u{9E1F}', "niao"),   // 鸟 — 鳥 (Trad)
    ('\u{9EA6}', "mai"),    // 麦
    ('\u{9EBB}', "ma"),     // 麻
    ('\u{9EC4}', "huang"),  // 黄
    ('\u{9ED1}', "hei"),    // 黑
    ('\u{9F13}', "gu"),     // 鼓
    ('\u{9F20}', "shu"),    // 鼠
    ('\u{9F3B}', "bi"),     // 鼻
    ('\u{9F50}', "qi"),     // 齐
    ('\u{9F7F}', "chi"),    // 齿
    ('\u{9F99}', "long"),   // 龙
];

/// Adapter that exposes [`Pinyin`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Chinese::phonetic_encoder`](crate::Chinese) hands back.
///
/// The adapter always returns `Some((key, None))` — Pinyin is a
/// single-key encoder — and considers all-separator input (empty or
/// pure whitespace / punctuation) as producing no phonetic key
/// (returns `None`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PinyinAdapter;

impl LanguagePhoneticEncoder for PinyinAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        let key = Pinyin.encode(word);
        if key.is_empty() {
            return None;
        }
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "pinyin-zh"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        Pinyin.encode(s)
    }

    #[test]
    fn table_is_sorted_by_scalar_value() {
        // The binary_search in `lookup` requires strict ascending
        // ordering. This test trips the moment someone inserts a new
        // entry at the wrong position.
        for w in PINYIN_TABLE.windows(2) {
            assert!(
                w[0].0 < w[1].0,
                "table not sorted at U+{:04X} -> U+{:04X}",
                w[0].0 as u32,
                w[1].0 as u32
            );
        }
    }

    #[test]
    fn table_has_no_duplicates() {
        // Sorted + no duplicates ↔ strict ascending, which the sort
        // test also checks; this is a paranoia guard.
        for w in PINYIN_TABLE.windows(2) {
            assert_ne!(w[0].0, w[1].0, "duplicate table entry for {:?}", w[0].0);
        }
    }

    #[test]
    fn table_covers_at_least_800_characters() {
        // The module docs advertise "~1000"; assert we're in the
        // 800-1500 ballpark. Both bounds are `const`, so we lift the
        // check into a `const` block to satisfy
        // `clippy::assertions_on_constants` — a mistuned bound then
        // trips at compile time rather than at first test run.
        const {
            assert!(
                PINYIN_TABLE_LEN >= 800 && PINYIN_TABLE_LEN <= 1500,
                "PINYIN_TABLE_LEN outside the advertised 800-1500 range",
            );
        }
    }

    #[test]
    fn all_pinyin_entries_are_ascii_lowercase() {
        for &(c, py) in PINYIN_TABLE {
            assert!(
                py.chars().all(|ch| ch.is_ascii_lowercase()),
                "non-ASCII-lowercase pinyin {py:?} for {c:?}",
            );
            assert!(!py.is_empty(), "empty pinyin for {c:?}");
        }
    }

    #[test]
    fn common_characters_encode() {
        assert_eq!(e("你好"), "ni hao");
        assert_eq!(e("中国"), "zhong guo");
        assert_eq!(e("北京"), "bei jing");
        assert_eq!(e("上海"), "shang hai");
        assert_eq!(e("我"), "wo");
        assert_eq!(e("是"), "shi");
        assert_eq!(e("的"), "de");
    }

    #[test]
    fn unknown_han_encodes_as_question_mark() {
        // U+9FEF is a Han character in the main CJK block, added in
        // Unicode 11.0 (2018) — extremely rare, unlikely to be in a
        // curated top-1000 frequency table.
        let c = '\u{9FEF}';
        let s: String = core::iter::once(c).collect();
        assert_eq!(classify(c), CharType::Han);
        assert_eq!(e(&s), "?");
    }

    #[test]
    fn ascii_letters_pass_through_lowercased() {
        assert_eq!(e("Hello"), "hello");
        assert_eq!(e("ABC"), "abc");
    }

    #[test]
    fn digits_pass_through() {
        assert_eq!(e("2026"), "2026");
    }

    #[test]
    fn han_and_ascii_are_space_separated() {
        // 中 zhong, 国 guo, then Latin run `abc`.
        assert_eq!(e("中国abc"), "zhong guo abc");
    }

    #[test]
    fn punctuation_collapses_adjacent_units_still_space_separated() {
        // The comma is a separator; the tokens on each side still
        // end up space-separated in the output.
        assert_eq!(e("你，好"), "ni hao");
        assert_eq!(e("中国。上海"), "zhong guo shang hai");
    }

    #[test]
    fn empty_input_yields_empty_string() {
        assert_eq!(e(""), "");
    }

    #[test]
    fn adapter_name_is_pinyin_zh() {
        assert_eq!(PinyinAdapter.name(), "pinyin-zh");
    }

    #[test]
    fn adapter_returns_some_for_han() {
        let out = PinyinAdapter.encode("你好");
        assert_eq!(out, Some((String::from("ni hao"), None)));
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(PinyinAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_returns_none_for_all_separators() {
        assert!(PinyinAdapter.encode("   ").is_none());
        assert!(PinyinAdapter.encode("。。。").is_none());
    }
}
