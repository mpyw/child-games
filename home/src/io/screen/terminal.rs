//! ターミナルに一言出すだけの `Screen`。

use kit::{Scene, Screen};

/// 場面の `caption` を出すだけ。動作確認用。
///
/// ANSI で四角を描く版もあったが、フォントやエミュレータに左右されて
/// 環境によって崩れる。見せる相手は2歳児でターミナルは見ないので、
/// ここは進行が追えれば足りる。
pub struct Terminal;

impl Screen for Terminal {
    fn show(&mut self, scene: Scene) {
        if !scene.caption.is_empty() {
            println!("{}", scene.caption);
        }
    }
}
