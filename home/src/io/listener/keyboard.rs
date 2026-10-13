//! 標準入力から答えを打つ `Listener`。音源と進行の確認用。

use std::io::Write;

use anyhow::Result;
use kit::{Heard, Listener};

/// 標準入力から答えを打つ。音源と進行の確認用。
pub struct Keyboard;

impl Listener for Keyboard {
    /// 語彙は出すだけ。打つのは人なので、何を打てば通るか見えるほうがよい。
    fn hear(&mut self, vocabulary: &[&str]) -> Result<Heard> {
        print!("  こたえは？（{}）> ", vocabulary.join(" / "));
        std::io::stdout().flush()?;
        let mut line = String::new();
        // 0 バイトは EOF。空行（改行だけ）とは別物で、こちらは終わり。
        if std::io::stdin().read_line(&mut line)? == 0 {
            println!();
            return Ok(Heard::Closed);
        }
        let line = line.trim();
        Ok(if line.is_empty() {
            Heard::Nothing
        } else {
            Heard::Said(line.to_string())
        })
    }
}
