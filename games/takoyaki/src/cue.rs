//! 鳴らす素材。文字列ではなく型で指す。

use kit::Sound;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    /// 遊んでいる間の曲。**この長さで1回が終わる。** ジュージュー焼ける音も
    /// ここに薄く混ぜてある（効果音の口は1本なので、鳴らし続けられない）。
    Song,
    /// 生地を流す。とろー。
    Pour,
    /// まだ焼けていないのをつついた。じゅっ。
    Sizzle,
    /// 焼けた。ピカッ。
    Ready,
    /// 裏返した。くるっ。
    Flip,
    /// つかんだ。
    Grab,
    /// 舟皿に乗った。ぽん、きらきら。
    Serve,
    /// 振り上げが弱くて落ちた。ぽとっ。
    Slip,
    /// 曲が終わったあとのお祝い。
    Fanfare,
}

impl Cue {
    pub const COUNT: usize = 9;

    /// 素材の名前の全部。`SOUNDS` と、起動時の検査に使う。
    pub const fn stems() -> [&'static str; Self::COUNT] {
        [
            "song", "pour", "sizzle", "ready", "flip", "grab", "serve", "slip", "fanfare",
        ]
    }

    pub const fn stem(self) -> &'static str {
        let s = Self::stems();
        match self {
            Cue::Song => s[0],
            Cue::Pour => s[1],
            Cue::Sizzle => s[2],
            Cue::Ready => s[3],
            Cue::Flip => s[4],
            Cue::Grab => s[5],
            Cue::Serve => s[6],
            Cue::Slip => s[7],
            Cue::Fanfare => s[8],
        }
    }

    pub const fn sound(self) -> Sound {
        Sound {
            game: crate::ID,
            name: self.stem(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cue_is_listed_once() {
        let all = [
            Cue::Song,
            Cue::Pour,
            Cue::Sizzle,
            Cue::Ready,
            Cue::Flip,
            Cue::Grab,
            Cue::Serve,
            Cue::Slip,
            Cue::Fanfare,
        ];
        let stems: Vec<_> = all.iter().map(|c| c.stem()).collect();
        assert_eq!(stems, Cue::stems());
    }
}
