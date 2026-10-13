//! 押されたものをどう受け取るか。
//!
//! メニューの選択、「もう1回」、遊びの途中でやめる、をここで受ける。
//! **声では受けない。** 歓声や物音で勝手に再開してしまうし、そもそも
//! 遊びを選ぶ・続ける・やめると決めるのは親の仕事なので、意思を持った
//! 操作に限る。
//!
//! マウス・キーボード・テレビのリモコン・標準入力は、供給元が違うだけで
//! 意味は同じ。だから口は1つにまとめて、意味（`Press`）だけを運ぶ。
//!
//! 実装は `home::io::control`。
//!
//! `Listener`（子どもが答える）とは別系統にしてある。あちらに混ぜると、
//! フォールバックが「ぜんぶ」に倒れないよう型で塞いだ意図が崩れるため。

/// 押されたもの。**どこから来たかは運ばない。**
///
/// 以前は「もう1回」と「やめる」で口を2本に分けていた。同じ口に `()` を
/// 流すと、やめる合図で再開してしまうため。意味を値に載せれば口は1本で
/// 済み、取り違えは型が起こさせない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// 決定。メニューの起動も「もう1回」もこれ。
    Ok,
    /// 戻る。遊びの途中なら「この回はやめる」、「もう1回」の画面なら
    /// メニューへ。
    Back,
    Up,
    Down,
    Left,
    Right,
    /// 番号で選ぶ（0 始まり）。ターミナルとキーボードの数字キー。
    Pick(usize),
    /// 画面のこの位置を押した。座標はキャンバス（`WIDTH` × `HEIGHT`）。
    ///
    /// 何が描いてあるかは描いた側しか知らないので、当たり判定は受け手が
    /// する。ウィンドウは位置を直すだけ。
    Point(i32, i32),
}

pub trait Control: Send {
    /// 溜まっているものを1つ取る。**待たない。** 無ければ `None`。
    fn poll(&mut self) -> Option<Press>;

    /// 1つ来るまで待つ。**`None` は入力が絶えた**（ウィンドウが閉じた、
    /// 標準入力が EOF になった）。続けられないなら終わりでよい。
    ///
    /// 待ちに上限は無い。押すのは親なので、テレビの前を離れていれば
    /// いつまででも待つ。
    fn wait(&mut self) -> Option<Press>;

    /// **遊びを途中でやめたいか。** 待たずにその場で答える。
    ///
    /// 歌っている最中に何度も呼ばれる。溜まっているぶんは**全部そこで
    /// 捨てる**（同じ合図で二度抜けないため）。見るのは「戻る」だけで、
    /// 十字キーや決定では止まらない。**押し間違いで歌を切らない。**
    fn stop_requested(&mut self) -> bool {
        let mut stop = false;
        while let Some(p) = self.poll() {
            stop |= p == Press::Back;
        }
        stop
    }

    /// 溜まっているぶんを捨てる。
    ///
    /// 待ちに入る前に呼ぶ。子どもは歌っている間もキーを叩くので、溜めた
    /// まま待ちに入るとフィナーレが鳴り終わった瞬間に再開してしまう。
    fn discard(&mut self) {
        while self.poll().is_some() {}
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;

    struct Queue(VecDeque<Press>);
    impl Control for Queue {
        fn poll(&mut self) -> Option<Press> {
            self.0.pop_front()
        }
        fn wait(&mut self) -> Option<Press> {
            self.0.pop_front()
        }
    }

    /// **戻る以外では止まらない。** そして見たぶんは全部捨てる。
    #[test]
    fn only_back_stops_and_everything_is_drained() {
        let mut q = Queue([Press::Up, Press::Ok, Press::Point(0, 0)].into());
        assert!(!q.stop_requested(), "十字キーや決定で止まった");
        assert!(q.0.is_empty(), "溜まっていたぶんを捨てていない");

        let mut q = Queue([Press::Ok, Press::Back, Press::Ok].into());
        assert!(q.stop_requested());
        assert!(!q.stop_requested(), "同じ合図で二度抜けた");
    }
}
