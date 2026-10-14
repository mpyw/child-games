//! つかんで、振り上げる。
//!
//! ```text
//! 空いている ─手が入った（0.1秒）─→ つかまれる ─強く振り上げた─→ 持ち上がった
//!     ↑                                 │
//!     └── 弱い / 振り上げない（5秒）── 逃げた ←┘
//! ```
//!
//! **何をつかむかは知らない。** 魚でもたこ焼きでも、「どこに手が入れば
//! つかんだことになるか（丸）」と「どこで振り上げを見るか（左右の長方形）」を
//! 渡せば同じに判定する。持ち上がったあと、逃げたあとどうするかは遊びが決める。
//!
//! **左右のどちらかが足りれば持ち上がる。** 両手を求めていた頃は、2歳児には
//! 厳しすぎた。左右に分けて見るのは、調整用の表示で強さを見せるため。

use std::time::Duration;

use kit::Rect;

use crate::{Flowed, Region};

/// 手が入ってから「つかんだ」とみなすまで。一瞬かすっただけではつかまない。
const GRAB: Duration = Duration::from_millis(100);
/// 手が入ったあと、動きがこれだけ途切れたら、入っていないことにする。
/// **手を止めても、すぐには離れたことにしない。** 見ているのは動きなので、
/// 手を止めると動きが消える。
const LINGER: Duration = Duration::from_millis(400);
/// つかんだまま振り上げないと、これで逃げる。
const HOLD: Duration = Duration::from_secs(5);
/// 振り上げを見る長さ。**この間に要る速さに届かなければ、弱いとみなして
/// 逃がす。**
const WINDOW: Duration = Duration::from_millis(500);
/// これだけの速さで上へ動いたら、振り上げを始めたとみなす（要る速さに対する割合）。
const STIR: f32 = 0.35;
/// 向きが分かった升目がこれより少なければ、振り上げは無かったことにする。
/// 1つ2つの升目は、カメラのざらつきでも向きを持つ。
const MIN_VECTORS: usize = 6;

/// 1つぶんの、見えたもの。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Seen {
    /// つかむ丸の中で動いた割合（0〜1）。
    pub grab: f32,
    /// 左側で、上へ動いた速さ（毎秒の画素）。向きが分からなければ 0。
    pub left: f32,
    /// 右側で、上へ動いた速さ。
    pub right: f32,
}

impl Seen {
    /// 絵の動きから、1つぶんの見えたものを出す。
    ///
    /// `reach` はつかむ丸 `(x, y, r)`、`zones` は振り上げを見る左右の範囲、
    /// `dt` は前の絵からの秒数。速さは「前の絵から何画素」なので、絵の間の
    /// 時間で割って毎秒にする。
    pub fn look(flowed: &Flowed, reach: (f32, f32, f32), zones: (Rect, Rect), dt: f32) -> Seen {
        let dt = dt.max(1.0 / 60.0);
        let speed = |r: Region| {
            if r.vectors >= MIN_VECTORS {
                r.rising / dt
            } else {
                0.0
            }
        };
        let (x, y, r) = reach;
        Seen {
            grab: flowed.ratio_in(x, y, r),
            left: speed(flowed.region(zones.0)),
            right: speed(flowed.region(zones.1)),
        }
    }
}

/// 振り上げの途中。左右の速さの最大を覚えておく。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swing {
    pub start: Duration,
    pub left: f32,
    pub right: f32,
}

/// つかまれているかどうか。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Grip {
    /// つかまれていない。`touched` は手が入った時刻と、最後に動きが見えた時刻。
    Free {
        touched: Option<(Duration, Duration)>,
    },
    /// つかまれている。振り上げを見ていれば `swing` がある。
    Held {
        since: Duration,
        swing: Option<Swing>,
    },
}

/// 何が起きたか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Grabbed,
    Lifted,
    Slipped,
}

impl Default for Grip {
    fn default() -> Self {
        Grip::Free { touched: None }
    }
}

impl Grip {
    /// つかまれているか。
    pub fn held(&self) -> bool {
        matches!(self, Grip::Held { .. })
    }

    /// 手が入りかけているか。入りかけのものは、遊びの側で動かさないほうがいい。
    pub fn touched(&self) -> bool {
        matches!(self, Grip::Free { touched: Some(_) }) || self.held()
    }

    /// 振り上げの途中なら、その強さ。調整用の表示に使う。
    pub fn swing(&self) -> Option<Swing> {
        match *self {
            Grip::Held { swing, .. } => swing,
            Grip::Free { .. } => None,
        }
    }

    /// 時刻を進める。`grab` はつかんだとみなす割合、`need` は持ち上がるのに
    /// 要る速さ（毎秒の画素）。`open` が偽なら、つかませない（逃げた直後など）。
    ///
    /// **持ち上がったら・逃げたら、空いている状態に戻る。** そのあとどうするかは
    /// 呼び出し側が決める。
    pub fn step(
        &mut self,
        now: Duration,
        s: Seen,
        grab: f32,
        need: f32,
        open: bool,
    ) -> Option<Outcome> {
        match *self {
            Grip::Free { touched } => {
                let touching = open && s.grab >= grab;
                let touched = match (touching, touched) {
                    (true, None) => Some((now, now)),
                    (true, Some((first, _))) => Some((first, now)),
                    (false, Some((first, last))) if open && now.saturating_sub(last) < LINGER => {
                        Some((first, last))
                    }
                    (false, _) => None,
                };
                match touched {
                    Some((first, last)) if last.saturating_sub(first) >= GRAB => {
                        *self = Grip::Held {
                            since: now,
                            swing: None,
                        };
                        Some(Outcome::Grabbed)
                    }
                    _ => {
                        *self = Grip::Free { touched };
                        None
                    }
                }
            }
            Grip::Held { since, swing } => {
                let swing = match swing {
                    None if s.left.max(s.right) >= need * STIR => Some(Swing {
                        start: now,
                        left: s.left,
                        right: s.right,
                    }),
                    Some(w) => Some(Swing {
                        left: w.left.max(s.left),
                        right: w.right.max(s.right),
                        ..w
                    }),
                    None => None,
                };
                let outcome = match swing {
                    Some(w) if w.left.max(w.right) >= need => Some(Outcome::Lifted),
                    Some(w) if now.saturating_sub(w.start) >= WINDOW => Some(Outcome::Slipped),
                    None if now.saturating_sub(since) >= HOLD => Some(Outcome::Slipped),
                    _ => None,
                };
                *self = match outcome {
                    Some(_) => Grip::default(),
                    None => Grip::Held { since, swing },
                };
                outcome
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 30.0;
    const GRAB_AT: f32 = 0.05;
    const NEED: f32 = 400.0;

    /// `secs` 秒ぶん、`seen` を見せながら進める。起きたことを集める。
    fn run(g: &mut Grip, from: Duration, secs: f32, seen: Seen) -> (Duration, Vec<Outcome>) {
        let mut t = from;
        let mut all = Vec::new();
        for _ in 0..(secs / FRAME).round() as usize {
            t += Duration::from_secs_f32(FRAME);
            all.extend(g.step(t, seen, GRAB_AT, NEED, true));
        }
        (t, all)
    }

    fn touch() -> Seen {
        Seen {
            grab: 0.5,
            ..Seen::default()
        }
    }

    fn held() -> (Grip, Duration) {
        let mut g = Grip::default();
        let (t, ev) = run(&mut g, Duration::ZERO, 0.3, touch());
        assert_eq!(ev, [Outcome::Grabbed]);
        (g, t)
    }

    #[test]
    fn a_brush_is_not_a_grab() {
        let mut g = Grip::default();
        let (t, ev) = run(&mut g, Duration::ZERO, FRAME * 2.0, touch());
        assert!(ev.is_empty());
        let (_, ev) = run(&mut g, t, 0.6, Seen::default());
        assert!(ev.is_empty());
        assert!(!g.touched(), "離れたのに入りかけのまま");
    }

    #[test]
    fn a_pausing_hand_still_grabs() {
        let mut g = Grip::default();
        let (t, _) = run(&mut g, Duration::ZERO, FRAME * 3.0, touch());
        let (t, _) = run(&mut g, t, 0.2, Seen::default());
        let (_, ev) = run(&mut g, t, FRAME * 2.0, touch());
        assert_eq!(ev, [Outcome::Grabbed]);
    }

    /// 左でも右でも、片手で強く振り上げれば持ち上がる。
    #[test]
    fn one_strong_hand_lifts() {
        for (left, right) in [(600.0, 0.0), (0.0, 600.0)] {
            let (mut g, t) = held();
            let t = t + Duration::from_secs_f32(FRAME);
            let s = Seen {
                grab: 0.5,
                left,
                right,
            };
            assert_eq!(g.step(t, s, GRAB_AT, NEED, true), Some(Outcome::Lifted));
            assert_eq!(g, Grip::default(), "持ち上がったら空く");
        }
    }

    #[test]
    fn a_weak_swing_slips() {
        let (mut g, t) = held();
        let weak = Seen {
            grab: 0.5,
            left: 200.0,
            right: 200.0,
        };
        let (_, ev) = run(&mut g, t, 0.7, weak);
        // 手を入れたままなので、逃げたあとまたつかむ。止めるのは呼び出し側。
        assert_eq!(ev.first(), Some(&Outcome::Slipped));
    }

    #[test]
    fn holding_still_too_long_slips() {
        let (mut g, t) = held();
        let (t, ev) = run(&mut g, t, 4.5, touch());
        assert!(ev.is_empty());
        let (_, ev) = run(&mut g, t, 1.0, touch());
        // 逃げたあとも手を入れ続けると、またつかむ。止めるのは呼び出し側。
        assert_eq!(ev.first(), Some(&Outcome::Slipped));
    }

    /// 閉じているときはつかめない。入りかけも消える。
    #[test]
    fn a_closed_grip_cannot_be_grabbed() {
        let mut g = Grip::default();
        let mut t = Duration::ZERO;
        for _ in 0..20 {
            t += Duration::from_secs_f32(FRAME);
            assert_eq!(g.step(t, touch(), GRAB_AT, NEED, false), None);
        }
        assert!(!g.touched());
    }
}
