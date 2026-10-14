//! この遊びのつまみ。`config.toml` の `[games.takoyaki]`。
//!
//! **読み込むのは `home`。** ここに置くのは型と既定値と検査だけで、
//! ファイルにも環境変数にも触らない。

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// 片面が焼けるまでの秒数の下限と上限。穴ごとに、この間でばらつかせる。
    pub cook_min: f32,
    pub cook_max: f32,
    /// 明るさがこれだけ変わったら「動いた」とみなす（1〜255）。
    pub threshold: u8,
    /// 穴の中で、これだけの割合が動いたら「つついた」とみなす（0〜1）。
    ///
    /// **低く取る。** 見ているのは明るさが変わった升目なので、指の縁しか数に
    /// 入らない。
    pub poke_ratio: f32,
    /// 穴の中で、これだけの割合が動いたら「つかんだ」とみなす（0〜1）。
    pub grab_ratio: f32,
    /// 持ち上がる振り上げの速さ（キャンバスの画素 / 秒）。たこ焼きの左右
    /// どちらかが、これ以上で上へ動けば舟皿へ飛ぶ。落とすたびに下がる。
    pub swing_speed: f32,
    /// 動いた所、動きの向き、振り上げの強さを画面に出す。しきい値を決めるとき用。
    pub show_motion: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cook_min: 4.0,
            cook_max: 8.0,
            threshold: 16,
            poke_ratio: 0.04,
            grab_ratio: 0.04,
            swing_speed: 280.0,
            show_motion: false,
        }
    }
}

impl Config {
    /// **起動時に値の筋を通す。** 遊んでいる最中に気づくのが一番まずい。
    /// `section` はエラーに出す節の名前（`home` が付ける）。
    pub fn validate(&self, section: &str) -> Result<()> {
        let ok = |v: f32| v.is_finite() && (1.0..=30.0).contains(&v);
        if !ok(self.cook_min) || !ok(self.cook_max) || self.cook_min > self.cook_max {
            anyhow::bail!(
                "{section}.cook_min と cook_max は 1〜30 で、cook_min ≦ cook_max であること\
                 （いまは {} と {}）",
                self.cook_min,
                self.cook_max
            );
        }
        if self.threshold == 0 {
            anyhow::bail!(
                "{section}.threshold は1以上であること（0だとカメラのざらつきでつつき続ける）"
            );
        }
        for (name, v) in [
            ("poke_ratio", self.poke_ratio),
            ("grab_ratio", self.grab_ratio),
        ] {
            if !(v > 0.0 && v <= 1.0) {
                anyhow::bail!("{section}.{name} は 0 より大きく 1 以下であること（いまは {v}）");
            }
        }
        // 測れる速さには上限がある（vision の探す幅 × カメラの枚数）。ラズパイの
        // 毎秒13枚で 1040 画素。それを超えると永久に持ち上がらない。
        if !(50.0..=1000.0).contains(&self.swing_speed) {
            anyhow::bail!(
                "{section}.swing_speed は 50〜1000 であること（いまは {}）",
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
            cfg.validate("games.takoyaki").unwrap_err().to_string()
        };
        assert!(bad(|c| c.cook_min = 0.0).contains("games.takoyaki.cook_min"));
        assert!(bad(|c| c.cook_max = 2.0).contains("cook_max"));
        assert!(bad(|c| c.cook_max = f32::NAN).contains("cook_max"));
        assert!(bad(|c| c.threshold = 0).contains("threshold"));
        assert!(bad(|c| c.poke_ratio = 0.0).contains("poke_ratio"));
        assert!(bad(|c| c.grab_ratio = 2.0).contains("grab_ratio"));
        assert!(bad(|c| c.swing_speed = 5000.0).contains("swing_speed"));
    }

    #[test]
    fn accepts_the_shipped_defaults() {
        Config::default()
            .validate("games.takoyaki")
            .expect("既定値が自分の検証に落ちている");
    }
}
