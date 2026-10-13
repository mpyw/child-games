//! ウィンドウとリモコン（CEC）から届く合図で待つ実装。

use std::sync::mpsc::Receiver;

use kit::{Control, Press};

/// ウィンドウとリモコンから届く合図を受ける。
///
/// **送り手が全部落ちること（= ウィンドウが閉じられた）が、そのまま
/// 「終わり」の合図になる。** 終了の経路を別に用意しなくてよい。場面を
/// 送る側で `Disconnected` をゲームの終了と読んでいるのと、向きが逆なだけで
/// 同じ作り。
///
/// 口は1本。以前は「もう1回」と「やめる」で2本に分けていた（同じ口に `()`
/// を流すと、やめる合図で再開してしまうため）。意味を `Press` に載せた
/// ので、取り違えは起きない。
pub struct Channel(pub Receiver<Press>);

impl Control for Channel {
    /// **待たない。** 送り手が落ちていても `None` で返す。**それは終了の
    /// 合図で、こちらの担当ではない**（`wait` が `None` を返して畳む）。
    fn poll(&mut self) -> Option<Press> {
        self.0.try_recv().ok()
    }

    fn wait(&mut self) -> Option<Press> {
        self.0.recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **やめる合図で再開してはいけない。** 口を1本にしたので、ここが
    /// 崩れていないことを押さえる。
    #[test]
    fn stopping_is_not_continuing() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut c = Channel(rx);

        assert!(!c.stop_requested(), "何も来ていない");
        tx.send(Press::Back).unwrap();
        tx.send(Press::Back).unwrap();
        assert!(c.stop_requested(), "押されたら真");
        assert!(!c.stop_requested(), "溜まっていたぶんは捨てる");

        // 決定を送っても、やめる側は動かない。
        tx.send(Press::Ok).unwrap();
        assert!(!c.stop_requested());
    }

    #[test]
    fn channel_ends_when_the_sender_drops() {
        let (tx, rx) = std::sync::mpsc::channel::<Press>();
        let mut c = Channel(rx);
        drop(tx); // ウィンドウが閉じられた
        assert_eq!(c.wait(), None, "送り手が居なくなったら終わり");
    }

    #[test]
    fn discarding_drops_what_piled_up_while_playing() {
        // 歌っている最中の連打が残ったままだと、フィナーレが鳴り終わった
        // 瞬間に再開してしまう。待ちに入る前のぶんは捨てること。
        let (tx, rx) = std::sync::mpsc::channel();
        let mut c = Channel(rx);
        for _ in 0..5 {
            tx.send(Press::Ok).unwrap();
        }
        drop(tx);
        c.discard();
        assert_eq!(c.wait(), None, "溜まっていたぶんで再開してはいけない");
    }

    #[test]
    fn channel_hands_over_what_is_pressed_while_waiting() {
        use std::time::Duration;

        let (tx, rx) = std::sync::mpsc::channel();
        let mut c = Channel(rx);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(25));
            let _ = tx.send(Press::Left);
        });
        assert_eq!(c.wait(), Some(Press::Left));
    }
}
