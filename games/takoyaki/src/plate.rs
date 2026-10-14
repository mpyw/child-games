//! たこ焼き器。6つの穴それぞれの、焼ける・裏返す・持ち上げるの移り変わり。
//!
//! **見えたものと時刻は外から受け取る。** 絵から「穴の中で動いた割合」と左右の
//! 「上向きの速さ」を出すのは呼び出し側（`vision::Seen::look`）。ここはそれを
//! 受け取って穴をどうするか決めるだけなので、絵を組まずに確かめられる。
//!
//! ```text
//! 空 ─→ 生地を流す ─→ 片面が焼けていく ─→ 焼けた ─つつく→ 裏返る
//!  ↑                     （つついても揺れるだけ）              ↓
//!  │                                               反対側が焼けていく
//!  │                                                         ↓
//!  └── 舟皿へ飛ぶ ←─強く振り上げた─ つかむ ←───────────── 焼けた
//!                                    └─ 弱い → 落ちて戻る（また焼けた）
//! ```
//!
//! **焦げない。** 焼けたら、つつかれるまで・持ち上げられるまで、いつまでも待つ。

use std::time::Duration;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use vision::{Grip, Outcome, Seen};

use crate::config::Config;

/// 穴の真ん中（キャンバスの画素）。3個×2段。**大きく取る。** 2歳児の手は
/// 狙った所に行かない。下の段は画面の下端ぎりぎりまで寄せる。
pub const HOLES: [(f32, f32); 6] = [
    (330.0, 405.0),
    (640.0, 405.0),
    (950.0, 405.0),
    (330.0, 610.0),
    (640.0, 610.0),
    (950.0, 610.0),
];
/// 穴の大きさ。
pub const HOLE: f32 = 100.0;
/// たこ焼きの大きさ。
pub const BALL: f32 = 80.0;

/// 生地を流す絵の長さ。
pub const POUR: Duration = Duration::from_millis(600);
/// 裏返す絵の長さ。
pub const FLIP: Duration = Duration::from_millis(450);
/// 舟皿へ飛んでいく絵の長さ。
pub const FLIGHT: Duration = Duration::from_millis(900);
/// 落ちて戻る絵の長さ。
pub const DROP: Duration = Duration::from_millis(400);
/// 揺れる絵の長さ。まだ焼けていないのをつついたとき。
pub const JIGGLE: Duration = Duration::from_millis(350);
/// 舟皿へ飛んでから、次の生地を流すまで。
const REFILL: Duration = Duration::from_millis(1000);
/// 始めてすぐ、穴ごとに生地を流すのをずらす幅（秒）。いっせいに焼けないように。
const START: f32 = 1.5;
/// 落ちてから、またつかめるようになるまで。
const COOL: Duration = Duration::from_millis(1000);
/// つついたと見る、動きの続く長さ。一瞬かすっただけではつつかない。
const POKE: Duration = Duration::from_millis(60);
/// 動きがこれだけ途切れたら、つつくのをやめたことにする。
const POKE_GAP: Duration = Duration::from_millis(300);
/// 一度つついたら、同じ穴はこれだけ反応しない。1回のつつきで二度裏返らないように。
const POKE_REST: Duration = Duration::from_millis(700);

/// 落とすたびに、持ち上がる速さにこれを掛ける。
const EASE: f32 = 0.85;
/// 持ち上がる速さは、`swing_speed` のこの割合より下げない。
const FLOOR: f32 = 0.5;

/// 1つの穴の様子。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    /// 空いている。`until` になったら生地を流す。
    Empty { until: Duration },
    /// 生地を流している。
    Pour { at: Duration },
    /// 片面が焼けていく。
    CookA { start: Duration, cook: Duration },
    /// 片面が焼けた。つつかれるのを待つ。
    ReadyA { at: Duration },
    /// 裏返っている。
    Flip { at: Duration },
    /// 反対側が焼けていく。
    CookB { start: Duration, cook: Duration },
    /// 両面が焼けた。つかまれ、振り上げられるのを待つ。`open` より前はつかめない。
    ReadyB {
        at: Duration,
        grip: Grip,
        open: Duration,
    },
    /// 舟皿へ飛んでいく。
    Flying { at: Duration },
    /// 振り上げが弱くて落ちた。少し跳ねて戻る。
    Dropped { at: Duration },
}

/// 何が起きたか。音を鳴らすのに使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Pour,
    /// まだ焼けていないのをつついた。
    Sizzle,
    Ready,
    Flip,
    Grab,
    Serve,
    Slip,
}

/// 1つの穴。
#[derive(Debug, Clone, Copy)]
pub struct Hole {
    pub x: f32,
    pub y: f32,
    pub stage: Stage,
    /// つついている途中。`(手が入った時刻, 最後に動きが見えた時刻)`。
    poke: Option<(Duration, Duration)>,
    /// これより前は、つついても反応しない。
    rest: Duration,
    /// 最後に揺れた時刻。まだ焼けていないのをつついたとき。
    pub jiggled: Option<Duration>,
}

impl Hole {
    /// つつく・つかむを見る丸。
    pub fn reach(&self) -> (f32, f32, f32) {
        (self.x, self.y, HOLE)
    }

    /// 振り上げを見る、左と右の範囲。**穴の上まで伸ばす。** 振り上げた手は
    /// すぐ穴の所から出ていくので、穴の所だけを見ると速さを取り損ねる。
    pub fn zones(&self) -> (kit::Rect, kit::Rect) {
        let (w, top, bottom) = (HOLE * 1.5, self.y - 320.0, self.y + HOLE);
        let left = kit::Rect {
            x: (self.x - w) as i32,
            y: top as i32,
            w: w as i32,
            h: (bottom - top) as i32,
        };
        (
            left,
            kit::Rect {
                x: self.x as i32,
                ..left
            },
        )
    }

    /// 片面ぶんの焼け具合（0〜1）。焼けていく途中でなければ `None`。
    pub fn doneness(&self, now: Duration) -> Option<f32> {
        match self.stage {
            Stage::CookA { start, cook } | Stage::CookB { start, cook } => {
                Some((now.saturating_sub(start).as_secs_f32() / cook.as_secs_f32()).min(1.0))
            }
            _ => None,
        }
    }

    /// つついたか。動きが少し続いたら、1回つついたことにする。
    fn poked(&mut self, now: Duration, s: Seen, ratio: f32) -> bool {
        if now < self.rest {
            self.poke = None;
            return false;
        }
        let touching = s.grab >= ratio;
        self.poke = match (touching, self.poke) {
            (true, None) => Some((now, now)),
            (true, Some((first, _))) => Some((first, now)),
            (false, Some((first, last))) if now.saturating_sub(last) < POKE_GAP => {
                Some((first, last))
            }
            (false, _) => None,
        };
        match self.poke {
            Some((first, last)) if last.saturating_sub(first) >= POKE => {
                self.poke = None;
                self.rest = now + POKE_REST;
                true
            }
            _ => false,
        }
    }
}

pub struct Plate {
    holes: [Hole; 6],
    /// 舟皿に乗った数。
    served: usize,
    /// 続けて落とした数。乗ったら 0 に戻す。
    slips: u32,
    cook: (f32, f32),
    poke: f32,
    grab: f32,
    swing: f32,
    rng: StdRng,
}

impl Plate {
    pub fn new(cfg: &Config, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let holes = HOLES.map(|(x, y)| Hole {
            x,
            y,
            stage: Stage::Empty {
                until: Duration::from_secs_f32(rng.gen_range(0.0..START)),
            },
            poke: None,
            rest: Duration::ZERO,
            jiggled: None,
        });
        Self {
            holes,
            served: 0,
            slips: 0,
            cook: (cfg.cook_min, cfg.cook_max),
            poke: cfg.poke_ratio,
            grab: cfg.grab_ratio,
            swing: cfg.swing_speed,
            rng,
        }
    }

    pub fn holes(&self) -> &[Hole; 6] {
        &self.holes
    }

    /// 舟皿に乗った数。**飛んでいる途中のものも数える。** 曲が終わった瞬間に
    /// 舟皿へ向かっていたものも、焼けたことには変わりない。
    pub fn served(&self) -> usize {
        self.served
            + self
                .holes
                .iter()
                .filter(|h| matches!(h.stage, Stage::Flying { .. }))
                .count()
    }

    /// 今、持ち上がるのに要る速さ（毎秒の画素）。**落とすたびに下がる。**
    pub fn need(&self) -> f32 {
        self.swing * EASE.powi(self.slips as i32).max(FLOOR)
    }

    fn cook_time(&mut self) -> Duration {
        let (lo, hi) = self.cook;
        Duration::from_secs_f32(if hi > lo {
            self.rng.gen_range(lo..hi)
        } else {
            lo
        })
    }

    /// 時刻を進める。`seen` は穴ごとに、見えたものを返す。
    pub fn step(&mut self, now: Duration, seen: impl Fn(&Hole) -> Seen) -> Vec<Event> {
        let mut events = Vec::new();
        let need = self.need();
        let mut served = 0;
        let mut slipped = 0;
        for i in 0..self.holes.len() {
            let s = seen(&self.holes[i]);
            let cook_a = self.cook_time();
            let cook_b = self.cook_time();
            let h = &mut self.holes[i];
            h.stage = match h.stage {
                Stage::Empty { until } if now >= until => {
                    events.push(Event::Pour);
                    Stage::Pour { at: now }
                }
                Stage::Pour { at } if now.saturating_sub(at) >= POUR => Stage::CookA {
                    start: now,
                    cook: cook_a,
                },
                Stage::CookA { start, cook } | Stage::CookB { start, cook }
                    if now.saturating_sub(start) >= cook =>
                {
                    events.push(Event::Ready);
                    match h.stage {
                        Stage::CookA { .. } => Stage::ReadyA { at: now },
                        _ => Stage::ReadyB {
                            at: now,
                            grip: Grip::default(),
                            open: now,
                        },
                    }
                }
                // **まだ焼けていなくても、つついたら揺れて鳴る。** 何も起きないと
                // 「無反応」になる。裏返しはしない。
                stage @ (Stage::CookA { .. } | Stage::CookB { .. }) => {
                    if h.poked(now, s, self.poke) {
                        h.jiggled = Some(now);
                        events.push(Event::Sizzle);
                    }
                    stage
                }
                Stage::ReadyA { at } => {
                    if h.poked(now, s, self.poke) {
                        events.push(Event::Flip);
                        Stage::Flip { at: now }
                    } else {
                        Stage::ReadyA { at }
                    }
                }
                Stage::Flip { at } if now.saturating_sub(at) >= FLIP => Stage::CookB {
                    start: now,
                    cook: cook_b,
                },
                Stage::ReadyB { at, mut grip, open } => {
                    match grip.step(now, s, self.grab, need, now >= open) {
                        Some(Outcome::Grabbed) => {
                            events.push(Event::Grab);
                            Stage::ReadyB { at, grip, open }
                        }
                        Some(Outcome::Lifted) => {
                            events.push(Event::Serve);
                            Stage::Flying { at: now }
                        }
                        Some(Outcome::Slipped) => {
                            events.push(Event::Slip);
                            slipped += 1;
                            Stage::Dropped { at: now }
                        }
                        None => Stage::ReadyB { at, grip, open },
                    }
                }
                Stage::Flying { at } if now.saturating_sub(at) >= FLIGHT => {
                    served += 1;
                    Stage::Empty {
                        until: now + REFILL,
                    }
                }
                Stage::Dropped { at } if now.saturating_sub(at) >= DROP => Stage::ReadyB {
                    at: now,
                    grip: Grip::default(),
                    open: now + COOL,
                },
                stage => stage,
            };
        }
        if events.contains(&Event::Serve) {
            self.slips = 0;
        }
        self.slips += slipped;
        self.served += served;
        events
    }

    /// 画面のこの位置を押した（マウス）。カメラが無いときに試すための口。
    ///
    /// 押した穴が、片面焼けていれば裏返し、両面焼けていれば舟皿へ飛ばし、
    /// 焼けていく途中なら揺らす。
    pub fn press(&mut self, now: Duration, x: i32, y: i32) -> Option<Event> {
        let (x, y) = (x as f32, y as f32);
        let h = self
            .holes
            .iter_mut()
            .find(|h| (h.x - x).hypot(h.y - y) <= HOLE)?;
        match h.stage {
            Stage::ReadyA { .. } => {
                h.stage = Stage::Flip { at: now };
                Some(Event::Flip)
            }
            Stage::ReadyB { .. } => {
                h.stage = Stage::Flying { at: now };
                self.slips = 0;
                Some(Event::Serve)
            }
            Stage::CookA { .. } | Stage::CookB { .. } => {
                h.jiggled = Some(now);
                Some(Event::Sizzle)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 30.0;

    /// どの穴にも同じものを見せながら `secs` 秒進める。起きたことを集める。
    fn run(p: &mut Plate, from: Duration, secs: f32, seen: Seen) -> (Duration, Vec<Event>) {
        run_at(p, from, secs, |_| seen)
    }

    /// 穴ごとに見せるものを変えながら進める。
    fn run_at(
        p: &mut Plate,
        from: Duration,
        secs: f32,
        seen: impl Fn(&Hole) -> Seen,
    ) -> (Duration, Vec<Event>) {
        let mut t = from;
        let mut all = Vec::new();
        for _ in 0..(secs / FRAME).round() as usize {
            t += Duration::from_secs_f32(FRAME);
            all.extend(p.step(t, &seen));
        }
        (t, all)
    }

    fn plate() -> Plate {
        Plate::new(&Config::default(), 5)
    }

    fn touch() -> Seen {
        Seen {
            grab: 0.5,
            ..Seen::default()
        }
    }

    /// `i` 番の穴だけを触る。
    fn only(i: usize, s: Seen) -> impl Fn(&Hole) -> Seen {
        let (x, y) = HOLES[i];
        move |h: &Hole| {
            if h.x == x && h.y == y {
                s
            } else {
                Seen::default()
            }
        }
    }

    /// `i` 番の穴が `pred` を満たすまで進める。
    fn until(p: &mut Plate, from: Duration, i: usize, pred: impl Fn(&Stage) -> bool) -> Duration {
        let mut t = from;
        while !pred(&p.holes()[i].stage) {
            t = run(p, t, FRAME, Seen::default()).0;
            assert!(
                t < Duration::from_secs(60),
                "{:?} のまま進まない",
                p.holes()[i].stage
            );
        }
        t
    }

    /// 始めると全部の穴に生地が入り、焼ける時刻は穴ごとにずれる。
    #[test]
    fn every_hole_starts_cooking_at_its_own_pace() {
        let mut p = plate();
        let (t, ev) = run(&mut p, Duration::ZERO, 2.5, Seen::default());
        assert_eq!(ev.iter().filter(|&&e| e == Event::Pour).count(), 6);
        let done: Vec<f32> = p.holes().iter().filter_map(|h| h.doneness(t)).collect();
        assert_eq!(done.len(), 6, "焼けていない穴がある");
        let (lo, hi) = done
            .iter()
            .fold((1.0f32, 0.0f32), |(lo, hi), &d| (lo.min(d), hi.max(d)));
        assert!(hi - lo > 0.05, "焼け具合が揃いすぎている: {done:?}");
    }

    /// **焦げない。** 焼けたまま放っておいても、いつまでも待っている。
    #[test]
    fn a_ready_side_waits_forever() {
        let mut p = plate();
        let t = until(&mut p, Duration::ZERO, 0, |s| {
            matches!(s, Stage::ReadyA { .. })
        });
        run(&mut p, t, 60.0, Seen::default());
        assert!(matches!(p.holes()[0].stage, Stage::ReadyA { .. }));
    }

    /// **まだ焼けていないのをつつくと、揺れて鳴る。** 裏返しはしない。
    #[test]
    fn poking_an_unready_one_jiggles() {
        let mut p = plate();
        let t = until(&mut p, Duration::ZERO, 0, |s| {
            matches!(s, Stage::CookA { .. })
        });
        let (_, ev) = run_at(&mut p, t, 0.2, only(0, touch()));
        assert_eq!(ev, [Event::Sizzle]);
        assert!(p.holes()[0].jiggled.is_some());
        assert!(matches!(p.holes()[0].stage, Stage::CookA { .. }));
    }

    /// 一瞬かすっただけでは、つつかない。
    #[test]
    fn a_brush_is_not_a_poke() {
        let mut p = plate();
        let t = until(&mut p, Duration::ZERO, 0, |s| {
            matches!(s, Stage::ReadyA { .. })
        });
        let (t, ev) = run_at(&mut p, t, FRAME, only(0, touch()));
        assert!(ev.is_empty());
        let (_, ev) = run(&mut p, t, 0.5, Seen::default());
        assert!(!ev.contains(&Event::Flip));
    }

    /// 片面が焼けたらつついて裏返し、反対側が焼けたら、つかんで振り上げると
    /// 舟皿へ飛ぶ。少しして空いた穴に次の生地が入る。
    #[test]
    fn the_whole_round_of_one_hole() {
        let mut p = plate();
        let t = until(&mut p, Duration::ZERO, 0, |s| {
            matches!(s, Stage::ReadyA { .. })
        });
        let (t, ev) = run_at(&mut p, t, 0.2, only(0, touch()));
        assert_eq!(ev, [Event::Flip], "裏返らない");
        // 1回のつつきで二度裏返らない。
        let (t, ev) = run_at(&mut p, t, 0.3, only(0, touch()));
        assert!(!ev.contains(&Event::Flip));
        let t = until(&mut p, t, 0, |s| matches!(s, Stage::ReadyB { .. }));
        let (t, ev) = run_at(&mut p, t, 0.3, only(0, touch()));
        assert_eq!(ev, [Event::Grab]);
        let swing = Seen {
            grab: 0.5,
            left: 600.0,
            right: 0.0,
        };
        let (t, ev) = run_at(&mut p, t, 0.1, only(0, swing));
        assert_eq!(ev, [Event::Serve]);
        assert_eq!(p.served(), 1, "飛んでいる途中も数える");
        let (t, _) = run(&mut p, t, 1.0, Seen::default());
        assert_eq!(p.served(), 1);
        assert!(matches!(p.holes()[0].stage, Stage::Empty { .. }));
        let (_, ev) = run(&mut p, t, 1.2, Seen::default());
        assert!(ev.contains(&Event::Pour), "次の生地が入らない");
    }

    /// 振り上げが弱いと落ちて戻り、また焼けたまま待つ。落とすたびに持ち上げ
    /// やすくなる。すぐにはつかめない。
    #[test]
    fn a_weak_lift_drops_back_and_eases() {
        let mut p = plate();
        let full = p.need();
        let t = until(&mut p, Duration::ZERO, 0, |s| {
            matches!(s, Stage::ReadyA { .. })
        });
        let (t, _) = run_at(&mut p, t, 0.2, only(0, touch()));
        let t = until(&mut p, t, 0, |s| matches!(s, Stage::ReadyB { .. }));
        let (t, _) = run_at(&mut p, t, 0.3, only(0, touch()));
        let weak = Seen {
            grab: 0.5,
            left: 150.0,
            right: 150.0,
        };
        let (t, ev) = run_at(&mut p, t, 0.5, only(0, weak));
        assert_eq!(ev, [Event::Slip]);
        assert!(p.need() < full);
        let (t, _) = run(&mut p, t, 0.5, Seen::default());
        assert!(matches!(p.holes()[0].stage, Stage::ReadyB { .. }));
        let (_, ev) = run_at(&mut p, t, 0.5, only(0, touch()));
        assert!(ev.is_empty(), "落ちた直後につかめた");
    }

    /// 1つの穴を触っても、ほかの穴は動かない。
    #[test]
    fn holes_are_independent() {
        let mut p = plate();
        let t = until(&mut p, Duration::ZERO, 2, |s| {
            matches!(s, Stage::ReadyA { .. })
        });
        let others: Vec<Stage> = (0..6)
            .filter(|&i| i != 2)
            .map(|i| p.holes()[i].stage)
            .collect();
        let (_, ev) = run_at(&mut p, t, 0.2, only(2, touch()));
        assert_eq!(ev.iter().filter(|&&e| e == Event::Flip).count(), 1);
        for (j, i) in (0..6).filter(|&i| i != 2).enumerate() {
            let before = others[j];
            let after = p.holes()[i].stage;
            assert!(
                !matches!(after, Stage::Flip { .. }) || matches!(before, Stage::ReadyA { .. }),
                "触っていない穴 {i} が裏返った"
            );
        }
    }

    /// マウスで押すと、焼け具合に応じて裏返す・舟皿へ飛ばす・揺らす。
    #[test]
    fn a_press_does_what_the_hole_needs() {
        let mut p = plate();
        let t = until(&mut p, Duration::ZERO, 0, |s| {
            matches!(s, Stage::CookA { .. })
        });
        let (x, y) = (HOLES[0].0 as i32, HOLES[0].1 as i32);
        assert_eq!(p.press(t, x, y), Some(Event::Sizzle));
        let t = until(&mut p, t, 0, |s| matches!(s, Stage::ReadyA { .. }));
        assert_eq!(p.press(t, x, y), Some(Event::Flip));
        let t = until(&mut p, t, 0, |s| matches!(s, Stage::ReadyB { .. }));
        assert_eq!(p.press(t, x, y), Some(Event::Serve));
        assert_eq!(p.press(t, -100, -100), None);
    }

    /// 穴はどれも画面に収まり、重ならない。
    #[test]
    fn holes_fit_and_do_not_overlap() {
        for (i, &(x, y)) in HOLES.iter().enumerate() {
            assert!(x - HOLE >= 0.0 && x + HOLE <= kit::WIDTH as f32);
            assert!(y - HOLE >= 0.0 && y + HOLE <= kit::HEIGHT as f32);
            for &(x2, y2) in &HOLES[i + 1..] {
                assert!((x - x2).hypot(y - y2) >= HOLE * 2.0, "{i} が重なる");
            }
        }
    }
}
