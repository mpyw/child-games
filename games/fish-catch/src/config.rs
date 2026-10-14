//! この遊びのつまみ。`config.toml` の `[games.fish_catch]`。
//!
//! **読み込むのは `home`。** ここに置くのは型と既定値と検査だけで、
//! ファイルにも環境変数にも触らない。

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// 池に同時に泳がせる魚の数。
    pub max_fish: usize,
    /// 明るさがこれだけ変わったら「動いた」とみなす（1〜255）。
    pub threshold: u8,
    /// 魚のまわりで、これだけの割合が動いたら「つかんだ」とみなす（0〜1）。
    ///
    /// **低く取る。** 見ているのは明るさが変わった升目なので、手の縁しか
    /// 数に入らない。魚のまわりで手を揺らしても、5〜10% にしかならない。
    pub grab_ratio: f32,
    /// 釣れる振り上げの速さ（キャンバスの画素 / 秒）。左右とも、これ以上で
    /// 上へ動けば釣れる。逃げるたびに下がる。
    pub swing_speed: f32,
    /// 動きの向きと、振り上げの強さを画面に出す。しきい値を決めるとき用。
    pub show_motion: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_fish: 3,
            threshold: 24,
            grab_ratio: 0.05,
            swing_speed: 420.0,
            show_motion: false,
        }
    }
}

impl Config {
    /// **起動時に値の筋を通す。** 遊んでいる最中に気づくのが一番まずい。
    /// `section` はエラーに出す節の名前（`home` が付ける）。
    pub fn validate(&self, section: &str) -> Result<()> {
        if !(1..=4).contains(&self.max_fish) {
            anyhow::bail!(
                "{section}.max_fish は 1〜4 であること（いまは {}）",
                self.max_fish
            );
        }
        if self.threshold == 0 {
            anyhow::bail!(
                "{section}.threshold は1以上であること（0だとカメラのざらつきでつかみ続ける）"
            );
        }
        if !(self.grab_ratio > 0.0 && self.grab_ratio <= 1.0) {
            anyhow::bail!(
                "{section}.grab_ratio は 0 より大きく 1 以下であること（いまは {}）",
                self.grab_ratio
            );
        }
        // 測れる速さには上限がある（vision の探す幅 × カメラの枚数）。ラズパイの
        // 毎秒13枚で 1040 画素。それを超えると永久に釣れない。
        if !(50.0..=1000.0).contains(&self.swing_speed) {
            anyhow::bail!(
                "{section}.swing_speed は 50〜1000 であること（いまは {}）。\
                 それより速い振り上げはカメラで測れない",
                self.swing_speed
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
            cfg.validate("games.fish_catch").unwrap_err().to_string()
        };
        assert!(bad(|c| c.max_fish = 0).contains("games.fish_catch.max_fish"));
        assert!(bad(|c| c.max_fish = 5).contains("max_fish"));
        assert!(bad(|c| c.threshold = 0).contains("threshold"));
        assert!(bad(|c| c.grab_ratio = 0.0).contains("grab_ratio"));
        assert!(bad(|c| c.swing_speed = 5000.0).contains("swing_speed"));
        assert!(bad(|c| c.swing_speed = f32::NAN).contains("swing_speed"));
    }

    #[test]
    fn accepts_the_shipped_defaults() {
        Config::default()
            .validate("games.fish_catch")
            .expect("既定値が自分の検証に落ちている");
    }
}
