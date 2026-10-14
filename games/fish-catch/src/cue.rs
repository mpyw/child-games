//! 鳴らす素材。文字列ではなく型で指す。

use kit::Sound;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    /// 遊んでいる間の曲。**この長さで1回が終わる。**
    Song,
    /// 魚をつかんだ。ピチピチ。
    Grab,
    /// 釣れた。ザバーンと、きらきら。
    Catch,
    /// 逃げられた。ツルン。
    Slip,
    /// 曲が終わったあとのお祝い。
    Fanfare,
}

impl Cue {
    pub const COUNT: usize = 5;

    /// 素材の名前の全部。`SOUNDS` と、起動時の検査に使う。
    pub const fn stems() -> [&'static str; Self::COUNT] {
        ["song", "grab", "catch", "slip", "fanfare"]
    }

    pub const fn stem(self) -> &'static str {
        let s = Self::stems();
        match self {
            Cue::Song => s[0],
            Cue::Grab => s[1],
            Cue::Catch => s[2],
            Cue::Slip => s[3],
            Cue::Fanfare => s[4],
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
        let all = [Cue::Song, Cue::Grab, Cue::Catch, Cue::Slip, Cue::Fanfare];
        let stems: Vec<_> = all.iter().map(|c| c.stem()).collect();
        assert_eq!(stems, Cue::stems());
    }
}
