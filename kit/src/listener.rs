//! 子どもの応答をどう受け取るか。
//!
//! 実装は `home::io::listener`。マイク（whisper）とキーボードの2通り。
//! キーボードは再生や進行を確かめるためのもので、whisper.cpp のビルドを
//! 待たずに動かせる。

use anyhow::Result;

/// 聞き取りの結果。
///
/// **「言わなかった」と「入力が絶えた」を分ける。** どちらも `None` に
/// 畳んでいた頃は、手打ちで Ctrl-D を押すとランダムな色を延々と鳴らし
/// 続けた。前者は遊びの側で何かに倒すべきで、後者は終わるべきもの。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    /// 何か言った。中身の解釈は遊びに任せる。
    Said(String),
    /// 何も言わなかった。
    Nothing,
    /// 入力そのものが絶えた。遊びを終える。
    Closed,
}

/// 応答を1回聞き取る。
///
/// **待つ長さは聞く側が決める**（`listen.max_seconds`）。録音の下拵え
/// （残響を無視する時間など）と組みで検査してあるので、遊びごとに
/// 変えられると、その検査が効かなくなる。
pub trait Listener {
    /// `vocabulary` は答えに出てくる語。whisper にあらかじめ教えて、
    /// 漢字変換や言い換えを抑える。**遊びごとに違うので、毎回渡す。**
    fn hear(&mut self, vocabulary: &[&str]) -> Result<Heard>;
}
