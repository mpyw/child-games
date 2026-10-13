//! この遊びのつまみ。`config.toml` の `[games.donna_iro]`。
//!
//! **読み込むのは `home`。** ここに置くのは型と既定値と検査だけで、
//! ファイルにも環境変数にも触らない。

use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// 何周に1回、区切り（ブリッジまたは間奏）を挟むか。
    pub insert_every: u32,
    /// フィナーレで色を差し替える間隔（ミリ秒）。
    flash_ms: u64,
    /// 認識結果の先頭いくつの区間を信用するか。
    ///
    /// whisper の出力をどこまで読むかの話だが、読むのは判定（`matcher.rs`）
    /// なのでこちらに置く。
    pub head_segments: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            insert_every: 3,
            flash_ms: 500,
            head_segments: 2,
        }
    }
}

impl Config {
    pub fn flash(&self) -> Duration {
        Duration::from_millis(self.flash_ms)
    }

    /// **起動時に値の筋を通す。** 遊んでいる最中に気づくのが一番まずい。
    ///
    /// `insert_every = 0` は割り算、`flash_ms = 0` はフィナーレのビジー
    /// ループになる。`section` はエラーに出す節の名前（`home` が付ける）。
    pub fn validate(&self, section: &str) -> Result<()> {
        if self.insert_every == 0 {
            anyhow::bail!("{section}.insert_every は1以上であること（0だと割り算で落ちる）");
        }
        if self.flash_ms == 0 {
            anyhow::bail!("{section}.flash_ms は1以上であること（0だとフィナーレが空回りする）");
        }
        if self.head_segments == 0 {
            anyhow::bail!("{section}.head_segments は1以上であること");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_values_that_would_panic_later() {
        let bad = |f: fn(&mut Config)| {
            let mut cfg = Config::default();
            f(&mut cfg);
            cfg.validate("games.donna_iro").unwrap_err().to_string()
        };
        // 割り算
        assert!(bad(|c| c.insert_every = 0).contains("games.donna_iro.insert_every"));
        // フィナーレが空回りする
        assert!(bad(|c| c.flash_ms = 0).contains("flash_ms"));
        assert!(bad(|c| c.head_segments = 0).contains("head_segments"));
    }

    #[test]
    fn accepts_the_shipped_defaults() {
        Config::default()
            .validate("games.donna_iro")
            .expect("既定値が自分の検証に落ちている");
    }
}
