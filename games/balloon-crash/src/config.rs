//! この遊びのつまみ。`config.toml` の `[games.balloon_crash]`。
//!
//! **読み込むのは `home`。** ここに置くのは型と既定値と検査だけで、
//! ファイルにも環境変数にも触らない。

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// 画面に同時に出す風船の数。
    pub max_balloons: usize,
    /// 明るさがこれだけ変わったら「動いた」とみなす（0〜255）。
    pub threshold: u8,
    /// 風船の中で、これだけの割合が動いたら割る（0〜1）。
    pub touch_ratio: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_balloons: 3,
            threshold: 28,
            touch_ratio: 0.2,
        }
    }
}

impl Config {
    /// **起動時に値の筋を通す。** 遊んでいる最中に気づくのが一番まずい。
    /// `section` はエラーに出す節の名前（`home` が付ける）。
    pub fn validate(&self, section: &str) -> Result<()> {
        if !(1..=6).contains(&self.max_balloons) {
            anyhow::bail!(
                "{section}.max_balloons は 1〜6 であること（いまは {}）。\
                 0 だと風船が出ず、多すぎると画面に収まらない",
                self.max_balloons
            );
        }
        if self.threshold == 0 {
            anyhow::bail!(
                "{section}.threshold は1以上であること（0だとカメラのざらつきで割れ続ける）"
            );
        }
        if !(self.touch_ratio > 0.0 && self.touch_ratio <= 1.0) {
            anyhow::bail!(
                "{section}.touch_ratio は 0 より大きく 1 以下であること（いまは {}）",
                self.touch_ratio
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_values_that_break_the_game() {
        let bad = |f: fn(&mut Config)| {
            let mut cfg = Config::default();
            f(&mut cfg);
            cfg.validate("games.balloon_crash").unwrap_err().to_string()
        };
        assert!(bad(|c| c.max_balloons = 0).contains("games.balloon_crash.max_balloons"));
        assert!(bad(|c| c.max_balloons = 7).contains("max_balloons"));
        assert!(bad(|c| c.threshold = 0).contains("threshold"));
        assert!(bad(|c| c.touch_ratio = 0.0).contains("touch_ratio"));
        assert!(bad(|c| c.touch_ratio = f32::NAN).contains("touch_ratio"));
    }

    #[test]
    fn accepts_the_shipped_defaults() {
        Config::default()
            .validate("games.balloon_crash")
            .expect("既定値が自分の検証に落ちている");
    }
}
