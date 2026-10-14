//! 鳴らす素材。文字列ではなく型で指す。

use kit::Sound;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    /// 遊んでいる間の曲。**この長さで1回が終わる。**
    Song,
    /// 割れた音。3通りを順に回す。同じ音が続くと単調になる。
    Pop(usize),
    /// 曲が終わったあとのお祝い。
    Fanfare,
}

/// 割れた音の数。
pub const POPS: usize = 3;

impl Cue {
    pub const COUNT: usize = 2 + POPS;

    /// 素材の名前の全部。`SOUNDS` と、起動時の検査に使う。
    pub const fn stems() -> [&'static str; Self::COUNT] {
        ["song", "pop-1", "pop-2", "pop-3", "fanfare"]
    }

    pub const fn stem(self) -> &'static str {
        let stems = Self::stems();
        match self {
            Cue::Song => stems[0],
            Cue::Pop(i) => stems[1 + i % POPS],
            Cue::Fanfare => stems[1 + POPS],
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

    /// 割れた音は3通りを回り、どれも一覧に載っている。
    #[test]
    fn every_cue_is_listed() {
        let all = Cue::stems();
        for c in [Cue::Song, Cue::Fanfare]
            .into_iter()
            .chain((0..POPS * 2).map(Cue::Pop))
        {
            assert!(all.contains(&c.stem()), "{c:?}");
        }
        assert_eq!(Cue::Pop(0).stem(), Cue::Pop(POPS).stem());
        assert_ne!(Cue::Pop(0).stem(), Cue::Pop(1).stem());
    }
}
