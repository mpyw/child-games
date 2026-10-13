//! 標準入力で待つ実装。ターミナル構成用。

use std::io::Write;

use kit::{Control, Press};

/// 標準入力を1行ずつ読む。ターミナル構成用。
///
/// 何を待っているか（メニューか「もう1回」か）は、直前に画面の側が
/// `caption` で出している。ここは読むだけ。
///
/// | 打つもの | 意味 |
/// | --- | --- |
/// | 空行（Enter） | 決定 |
/// | 数字 | その番号を選ぶ（1 始まり） |
/// | `b` | 戻る |
/// | EOF（Ctrl-D） | 終わり |
pub struct Stdin;

impl Control for Stdin {
    /// **ターミナルには割り込みの口が無い。** 標準入力は `wait` が
    /// 塞いでいて、歌っている間は読めない。途中でやめたいなら Ctrl-C でよい
    /// （テレビの前ではない構成なので）。
    fn poll(&mut self) -> Option<Press> {
        None
    }

    fn wait(&mut self) -> Option<Press> {
        loop {
            print!("  [Enter=けってい / 数字=えらぶ / b=もどる / Ctrl-D=おわり] > ");
            if std::io::stdout().flush().is_err() {
                return None;
            }
            let mut line = String::new();
            // read_line が 0 を返すのは EOF のときだけ。空行でも改行ぶんの 1 は返る。
            match std::io::stdin().read_line(&mut line) {
                Ok(n) if n > 0 => {}
                _ => return None,
            }
            // 読めないものは訊き直す。**黙って決定にしない。**
            if let Some(p) = parse(&line) {
                return Some(p);
            }
        }
    }
}

fn parse(line: &str) -> Option<Press> {
    match line.trim() {
        "" => Some(Press::Ok),
        "b" | "B" => Some(Press::Back),
        n => match n.parse::<usize>() {
            Ok(i) if i >= 1 => Some(Press::Pick(i - 1)),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_read_as_presses() {
        assert_eq!(parse("\n"), Some(Press::Ok));
        assert_eq!(parse("b\n"), Some(Press::Back));
        // 人に見せる番号は 1 始まり。
        assert_eq!(parse("1\n"), Some(Press::Pick(0)));
        assert_eq!(parse(" 2 \n"), Some(Press::Pick(1)));
        // 0 番は無い。打ち間違いを決定にしない。
        assert_eq!(parse("0\n"), None);
        assert_eq!(parse("あか\n"), None);
    }
}
