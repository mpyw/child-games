//! 池と魚。泳ぐ・つかまれる・釣れる・逃げるの移り変わり。
//!
//! **見えたものと時刻は外から受け取る。** 絵から「つかんだ割合」と左右の
//! 「上向きの速さ」を出すのは呼び出し側（`vision` を使う）。ここはそれを
//! 受け取って魚をどうするか決めるだけなので、絵を組まずに確かめられる。
//!
//! ```text
//! 泳ぐ ─手が入った─→ つかまれる ─強く振り上げた─→ 釣れる
//!  ↑                    │
//!  │                    ├─ 弱い / 振り上げない ─→ 逃げる
//!  └────────────────────┴──────────────────────────────────┘
//! ```

use std::time::Duration;

use kit::{Rect, Rgb, HEIGHT, WIDTH};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
pub use vision::Seen;
use vision::{Grip, Outcome};

use crate::config::Config;

/// 池にいるものの種類。**つかめる範囲は種類によらず同じ。** 見た目だけが違う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// ふつうの魚。色いろ。
    Fish,
    /// フグ。まんまる。
    Puffer,
    /// タコ。足がゆらゆら。
    Octopus,
    /// サメ。長くて大きい。たまにしか出ない。
    Shark,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Fish, Kind::Puffer, Kind::Octopus, Kind::Shark];

    /// 出る割合（百分率）。足して 100。
    const fn weight(self) -> u32 {
        match self {
            Kind::Fish => 55,
            Kind::Puffer => 18,
            Kind::Octopus => 15,
            Kind::Shark => 12,
        }
    }

    /// その種類で使う色。
    const fn colors(self) -> &'static [Rgb] {
        match self {
            Kind::Fish => &COLORS,
            Kind::Puffer => &[(255, 205, 60), (255, 160, 70)],
            Kind::Octopus => &[(235, 90, 80), (240, 120, 170)],
            Kind::Shark => &[(120, 140, 165)],
        }
    }
}

/// 池にいるもの1匹の見た目。釣ったものもこれで覚える。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Creature {
    pub kind: Kind,
    pub color: Rgb,
}

impl Creature {
    /// 種類ごとに1匹ずつ。1匹も釣れなかったときのお祝いに並べる。
    pub fn parade() -> Vec<Creature> {
        Kind::ALL
            .iter()
            .flat_map(|&kind| {
                kind.colors()
                    .iter()
                    .take(if kind == Kind::Fish { 3 } else { 1 })
                    .map(move |&color| Creature { kind, color })
            })
            .collect()
    }
}

/// 魚の色。離れたテレビでも見分けられる、はっきりした色だけ。
pub const COLORS: [Rgb; 6] = [
    (255, 140, 40),
    (230, 60, 60),
    (255, 205, 40),
    (80, 170, 240),
    (240, 120, 180),
    (120, 200, 110),
];

/// 水面の高さ。これより下が池。
pub const WATER: f32 = HEIGHT as f32 * 0.5;
/// 魚の体の幅と高さ（キャンバスの画素）。**大きく取る。** 2歳児の手は
/// 狙った所に行かない。
pub const BODY: (f32, f32) = (230.0, 110.0);

/// 魚が泳ぐ高さの帯。水面より下で、子どもの手が届く所。
///
/// 3匹が深さで重なりにくい幅を取る（体の高さ 110 画素 × 2 より広く）。
const DEPTH: (f32, f32) = (HEIGHT as f32 * 0.58, HEIGHT as f32 * 0.88);
/// 泳ぐ速さ（毎秒の画素）。目で追える速さにする。
const SWIM: f32 = 90.0;
/// 逃げた直後の速さ。慌てて泳いでいく。
const DASH: f32 = 260.0;
/// 放っておかれて出ていく速さ。**泳ぐより少し速く、逃げるより遅く。**
/// 遅いと池が空いたままになり、速いと逃げたように見える。
const LEAVE: f32 = 140.0;
/// 釣れたときの絵の長さ。魚が飛んでいく。
pub const FLIGHT: Duration = Duration::from_millis(1000);
/// 逃げたときの絵の長さ。跳ねて水に戻る。
pub const HOP: Duration = Duration::from_millis(500);
/// 逃げてから、またつかめるようになるまで。
const COOL: Duration = Duration::from_millis(1500);
/// 慌てて泳ぐ長さ。
const FLEE: Duration = Duration::from_millis(1200);
/// 釣れたあと、次の魚を出すまで。
const REFILL: Duration = Duration::from_millis(1200);
/// 次の魚を出すまでの間（泳がせ始め）。いっぺんに出すと、どれを狙うかで迷う。
const STAGGER: Duration = Duration::from_millis(1500);
/// **放っておかれた魚は、この間のどこかで出ていく**（秒）。空いたぶんは別の色の
/// 魚が入ってくる。同じ魚が往復し続けるだけだと、見ていて飽きる。魚ごとに
/// ずらすのは、いっせいに出ていかないように。
const STAY: (f32, f32) = (12.0, 18.0);

/// 逃げるたびに、釣れる速さにこれを掛ける。
const EASE: f32 = 0.85;
/// 釣れる速さは、`swing_speed` のこの割合より下げない。
const FLOOR: f32 = 0.5;

/// 魚の様子。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    /// 池にいる。つかまれていなければ泳ぎ、つかまれていれば止まって跳ねる。
    /// つかむ・振り上げるの判定は `vision::Grip` に任せる。
    Live(Grip),
    /// 釣れた。飛んでいく。
    Caught { at: Duration },
    /// 逃げた。跳ねて水に戻り、慌てて泳いでいく。
    Slipped { at: Duration },
}

/// 何が起きたか。音を鳴らすのに使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Grab,
    Catch,
    Slip,
}

#[derive(Debug, Clone)]
pub struct Fish {
    pub x: f32,
    pub y: f32,
    pub look: Creature,
    /// 右向きなら 1、左向きなら -1。
    pub facing: f32,
    pub state: State,
    /// これより前はつかめない（逃げた直後）。
    cool: Duration,
    /// この時刻まで誰もつかまなければ、出ていく。
    stay: Duration,
    /// 出ていく途中。**端で折り返さず、画面の外へ抜けたら消える。** 途中でも
    /// つかめば、普通に釣れる。
    pub leaving: bool,
    /// 何匹目に出した魚か。入れ替わったかを見分けるのに使う（テスト）。
    #[cfg_attr(not(test), allow(dead_code))]
    pub id: u32,
    /// 揺れ方をずらす。
    pub phase: f32,
}

pub struct Pond {
    fish: Vec<Fish>,
    caught: Vec<Creature>,
    /// 続けて逃げた数。釣れたら 0 に戻す。
    slips: u32,
    next_spawn: Duration,
    /// これまでに出した魚の数。
    spawned: u32,
    max: usize,
    grab: f32,
    swing: f32,
    rng: StdRng,
}

impl Fish {
    /// つかんだかを見る丸。体より一回り大きい。
    pub fn reach(&self) -> (f32, f32, f32) {
        (self.x, self.y, BODY.0 * 0.8)
    }

    /// 振り上げを見る、左と右の範囲。**魚の上まで伸ばす。** 振り上げた手は
    /// すぐ魚の所から出ていくので、魚の所だけを見ると速さを取り損ねる。
    pub fn zones(&self) -> (Rect, Rect) {
        let (w, top, bottom) = (BODY.0 * 1.1, self.y - 320.0, self.y + BODY.1);
        let left = Rect {
            x: (self.x - w) as i32,
            y: top as i32,
            w: w as i32,
            h: (bottom - top) as i32,
        };
        (
            left,
            Rect {
                x: self.x as i32,
                ..left
            },
        )
    }
}

impl Pond {
    pub fn new(cfg: &Config, seed: u64) -> Self {
        Self {
            fish: Vec::new(),
            caught: Vec::new(),
            slips: 0,
            next_spawn: Duration::ZERO,
            spawned: 0,
            max: cfg.max_fish,
            grab: cfg.grab_ratio,
            swing: cfg.swing_speed,
            rng: StdRng::seed_from_u64(seed),
        }
    }

    pub fn fish(&self) -> &[Fish] {
        &self.fish
    }

    /// バケツに入った魚。釣った順。
    pub fn caught(&self) -> &[Creature] {
        &self.caught
    }

    /// 釣った魚の全部。**飛んでいる途中のものも数える。** 曲が終わった
    /// 瞬間にバケツへ向かっていた魚も、釣ったことには変わりない。
    pub fn haul(&self) -> Vec<Creature> {
        let flying = self
            .fish
            .iter()
            .filter(|f| matches!(f.state, State::Caught { .. }))
            .map(|f| f.look);
        self.caught.iter().copied().chain(flying).collect()
    }

    /// 今、釣れるのに要る速さ（毎秒の画素）。**逃げるたびに下がる。**
    pub fn need(&self) -> f32 {
        self.swing * EASE.powi(self.slips as i32).max(FLOOR)
    }

    /// 時刻を進める。`seen` は魚ごとに、見えたものを返す。
    pub fn step(&mut self, now: Duration, dt: f32, seen: impl Fn(&Fish) -> Seen) -> Vec<Event> {
        let mut events = Vec::new();
        let need = self.need();
        let mut slipped = 0;
        let mut landed = Vec::new();
        let mut gone = Vec::new();
        let mut rng = StdRng::seed_from_u64(self.rng.gen());
        let mut stay = || now + Duration::from_secs_f32(rng.gen_range(STAY.0..STAY.1));
        // **出ていくのは1匹ずつ。** 重なると池が空っぽに見える。順番が来ない
        // 魚は、前の魚が抜けるまで待つ。
        let mut leaving = self.fish.iter().any(|f| f.leaving);
        for (i, f) in self.fish.iter_mut().enumerate() {
            let s = seen(f);
            match f.state {
                State::Live(mut grip) => {
                    if !grip.held() {
                        // 放っておかれたら、近いほうの端へゆっくり出ていく。
                        if !leaving && !f.leaving && !grip.touched() && now >= f.stay {
                            f.leaving = true;
                            leaving = true;
                            f.facing = if f.x < WIDTH as f32 / 2.0 { -1.0 } else { 1.0 };
                        }
                        let speed = if now < f.cool && f.cool.saturating_sub(now) > COOL - FLEE {
                            DASH
                        } else if f.leaving {
                            LEAVE
                        } else {
                            SWIM
                        };
                        f.x += f.facing * speed * dt;
                        // 端で向きを変える。外から入ってくるときと、出ていくときは変えない。
                        if !f.leaving
                            && ((f.x < WIDTH as f32 * 0.1 && f.facing < 0.0)
                                || (f.x > WIDTH as f32 * 0.9 && f.facing > 0.0))
                        {
                            f.facing = -f.facing;
                        }
                        if f.leaving && (f.x < -BODY.0 || f.x > WIDTH as f32 + BODY.0) {
                            gone.push(i);
                            continue;
                        }
                    }
                    let open = f.x > 0.0 && f.x < WIDTH as f32 && now >= f.cool;
                    f.state = match grip.step(now, s, self.grab, need, open) {
                        Some(Outcome::Grabbed) => {
                            events.push(Event::Grab);
                            f.leaving = false;
                            State::Live(grip)
                        }
                        Some(Outcome::Lifted) => {
                            events.push(Event::Catch);
                            State::Caught { at: now }
                        }
                        Some(Outcome::Slipped) => {
                            events.push(Event::Slip);
                            slipped += 1;
                            State::Slipped { at: now }
                        }
                        None => State::Live(grip),
                    };
                }
                State::Caught { at } => {
                    if now.saturating_sub(at) >= FLIGHT {
                        landed.push(i);
                    }
                }
                State::Slipped { at } => {
                    if now.saturating_sub(at) >= HOP {
                        // 跳ねて戻ったら、近いほうの端へ慌てて泳いでいく。
                        f.facing = if f.x < WIDTH as f32 / 2.0 { -1.0 } else { 1.0 };
                        f.cool = now + COOL;
                        f.stay = stay();
                        f.state = State::Live(Grip::default());
                    }
                }
            }
        }
        if events.contains(&Event::Catch) {
            self.slips = 0;
            self.next_spawn = self.next_spawn.max(now + REFILL);
        }
        self.slips += slipped;
        // 釣れた魚はバケツへ。出ていった魚は黙って消す。どちらも、空いた
        // ぶんは下の `spawn` が埋める。
        let mut out: Vec<(usize, bool)> = landed
            .into_iter()
            .map(|i| (i, true))
            .chain(gone.into_iter().map(|i| (i, false)))
            .collect();
        // 後ろから消す。前から消すと、残りの位置がずれる。
        out.sort_unstable_by_key(|&(i, _)| std::cmp::Reverse(i));
        for (i, caught) in out {
            let f = self.fish.remove(i);
            if caught {
                self.caught.push(f.look);
            }
        }
        if self.fish.len() < self.max && now >= self.next_spawn {
            self.spawn(now);
            self.next_spawn = now + STAGGER;
        }
        events
    }

    /// 左右どちらかの外から1匹出す。内向きに泳いでくる。
    fn spawn(&mut self, now: Duration) {
        let from_left = self.rng.gen_bool(0.5);
        let x = if from_left {
            -BODY.0
        } else {
            WIDTH as f32 + BODY.0
        };
        // 他の魚と同じ深さに重ならないよう、空いている高さを選ぶ。
        let mut y = self.rng.gen_range(DEPTH.0..DEPTH.1);
        for _ in 0..8 {
            if self.fish.iter().all(|f| (f.y - y).abs() > BODY.1 * 0.9) {
                break;
            }
            y = self.rng.gen_range(DEPTH.0..DEPTH.1);
        }
        let look = self.pick();
        self.fish.push(Fish {
            x,
            y,
            look,
            facing: if from_left { 1.0 } else { -1.0 },
            state: State::Live(Grip::default()),
            cool: Duration::ZERO,
            stay: now + Duration::from_secs_f32(self.rng.gen_range(STAY.0..STAY.1)),
            leaving: false,
            id: self.spawned,
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
        });
        self.spawned += 1;
    }

    /// 次に出すものを選ぶ。種類は出る割合で、色はその種類の中から。
    fn pick(&mut self) -> Creature {
        let mut roll = self.rng.gen_range(0..100);
        let kind = Kind::ALL
            .into_iter()
            .find(|k| {
                if roll < k.weight() {
                    true
                } else {
                    roll -= k.weight();
                    false
                }
            })
            .unwrap_or(Kind::Fish);
        let colors = kind.colors();
        Creature {
            kind,
            color: colors[self.rng.gen_range(0..colors.len())],
        }
    }

    /// 画面のこの位置を押した（マウス）。**そこにいる魚をすぐ釣る。**
    /// カメラが無いときに試すための口。
    pub fn poke(&mut self, now: Duration, x: i32, y: i32) -> bool {
        let (x, y) = (x as f32, y as f32);
        let hit = self.fish.iter_mut().find(|f| {
            matches!(f.state, State::Live(_))
                && (f.x - x).abs() < BODY.0 * 0.7
                && (f.y - y).abs() < BODY.1 * 1.2
        });
        match hit {
            Some(f) => {
                f.state = State::Caught { at: now };
                self.slips = 0;
                self.next_spawn = self.next_spawn.max(now + REFILL);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 30.0;

    /// 1匹だけの池。
    fn pond() -> Pond {
        Pond::new(
            &Config {
                max_fish: 1,
                ..Config::default()
            },
            11,
        )
    }

    /// `secs` 秒ぶん、`seen` を見せながら 30fps で進める。起きたことを集める。
    fn run(p: &mut Pond, from: Duration, secs: f32, seen: Seen) -> (Duration, Vec<Event>) {
        let mut t = from;
        let mut all = Vec::new();
        for _ in 0..(secs / FRAME).round() as usize {
            t += Duration::from_secs_f32(FRAME);
            all.extend(p.step(t, FRAME, |_| seen));
        }
        (t, all)
    }

    /// 魚が画面に入ってくるまで泳がせて、つかませる。
    fn held(p: &mut Pond) -> Duration {
        let (t, _) = run(p, Duration::ZERO, 4.0, Seen::default());
        let touch = Seen {
            grab: 0.5,
            ..Seen::default()
        };
        let (t, ev) = run(p, t, 0.3, touch);
        assert_eq!(ev, [Event::Grab], "つかめていない");
        assert!(matches!(p.fish()[0].state, State::Live(g) if g.held()));
        t
    }

    fn both(speed: f32) -> Seen {
        Seen {
            grab: 0.5,
            left: speed,
            right: speed,
        }
    }

    /// 泳いで端で折り返し、泳ぐ帯からはみ出さない。
    #[test]
    fn fish_swim_back_and_forth_within_the_pond() {
        let mut p = Pond::new(&Config::default(), 3);
        let (mut t, ev) = run(&mut p, Duration::ZERO, 8.0, Seen::default());
        assert!(ev.is_empty());
        assert_eq!(p.fish().len(), Config::default().max_fish);
        for _ in 0..300 {
            t = run(&mut p, t, FRAME, Seen::default()).0;
            for f in p.fish() {
                assert!(
                    f.x > -BODY.0 * 1.1 && f.x < WIDTH as f32 + BODY.0 * 1.1,
                    "{f:?}"
                );
                assert!(f.y >= DEPTH.0 && f.y <= DEPTH.1, "{f:?}");
                assert!(f.y - BODY.1 / 2.0 > WATER, "水の上にはみ出した: {f:?}");
            }
        }
    }

    /// **放っておかれた魚は出ていき、別の魚と入れ替わる。** 釣った数には入らない。
    /// 泳いでいる数は保たれる。
    #[test]
    fn idle_fish_leave_and_are_replaced() {
        let mut p = Pond::new(&Config::default(), 9);
        let (t, _) = run(&mut p, Duration::ZERO, 6.0, Seen::default());
        let first: Vec<u32> = p.fish().iter().map(|f| f.id).collect();
        let mut t = t;
        let mut saw_leaving = false;
        for _ in 0..(40.0 / FRAME) as usize {
            t = run(&mut p, t, FRAME, Seen::default()).0;
            saw_leaving |= p.fish().iter().any(|f| f.leaving);
        }
        assert!(saw_leaving, "出ていく魚がいない");
        let now: Vec<u32> = p.fish().iter().map(|f| f.id).collect();
        assert!(
            first.iter().all(|id| !now.contains(id)),
            "入れ替わっていない: {first:?} → {now:?}"
        );
        assert!(p.fish().len() >= Config::default().max_fish - 1);
        assert!(p.caught().is_empty(), "出ていった魚を釣ったことにした");
    }

    /// **いっせいに出ていかない。** 出ていく時刻は魚ごとにずれる。
    #[test]
    fn fish_do_not_all_leave_at_once() {
        let mut p = Pond::new(&Config::default(), 4);
        let mut t = Duration::ZERO;
        let mut most = 0;
        for _ in 0..(40.0 / FRAME) as usize {
            t = run(&mut p, t, FRAME, Seen::default()).0;
            most = most.max(p.fish().iter().filter(|f| f.leaving).count());
        }
        assert_eq!(most, 1, "2匹以上が同時に出ていった");
    }

    /// 出ていく途中でも、つかめば釣れる。つかまれたら出ていくのをやめる。
    #[test]
    fn a_leaving_fish_can_still_be_caught() {
        let mut p = pond();
        let mut t = Duration::ZERO;
        while !p
            .fish()
            .first()
            .is_some_and(|f| f.leaving && f.x > 50.0 && f.x < WIDTH as f32 - 50.0)
        {
            t = run(&mut p, t, FRAME, Seen::default()).0;
            assert!(t < Duration::from_secs(40), "出ていかない");
        }
        let touch = Seen {
            grab: 0.5,
            ..Seen::default()
        };
        let (t, ev) = run(&mut p, t, 0.3, touch);
        assert_eq!(ev, [Event::Grab]);
        assert!(!p.fish()[0].leaving);
        let (_, ev) = run(&mut p, t, 0.1, both(600.0));
        assert_eq!(ev, [Event::Catch]);
    }

    /// **一瞬かすっただけではつかまない。**
    #[test]
    fn a_brush_is_not_a_grab() {
        let mut p = pond();
        let (t, _) = run(&mut p, Duration::ZERO, 4.0, Seen::default());
        let touch = Seen {
            grab: 0.5,
            ..Seen::default()
        };
        let (t, ev) = run(&mut p, t, FRAME * 2.0, touch);
        assert!(ev.is_empty());
        let (_, ev) = run(&mut p, t, 0.5, Seen::default());
        assert!(ev.is_empty(), "かすっただけでつかんだ");
    }

    /// **手が少し止まっても、つかめる。** 見ているのは動きなので、魚の上で
    /// 手を止めると途切れる。途切れてもすぐには離れたことにしない。
    #[test]
    fn a_pausing_hand_still_grabs() {
        let mut p = pond();
        let (t, _) = run(&mut p, Duration::ZERO, 4.0, Seen::default());
        let touch = Seen {
            grab: 0.5,
            ..Seen::default()
        };
        let (t, _) = run(&mut p, t, FRAME * 3.0, touch);
        let (t, _) = run(&mut p, t, 0.2, Seen::default());
        let (_, ev) = run(&mut p, t, FRAME * 2.0, touch);
        assert_eq!(ev, [Event::Grab]);
    }

    /// 両手で強く振り上げれば釣れて、少しして池から消え、釣った数に入る。
    #[test]
    fn a_strong_swing_with_both_hands_catches() {
        let mut p = pond();
        let t = held(&mut p);
        let (t, ev) = run(&mut p, t, 0.1, both(600.0));
        assert_eq!(ev, [Event::Catch]);
        let look = p.fish()[0].look;
        run(&mut p, t, 1.1, Seen::default());
        assert_eq!(p.caught(), &[look]);
    }

    /// **片手だけでも、強く振り上げれば釣れる。** 左でも右でも。
    #[test]
    fn one_hand_is_enough() {
        for (left, right) in [(600.0, 0.0), (0.0, 600.0)] {
            let mut p = pond();
            let t = held(&mut p);
            let one = Seen {
                grab: 0.5,
                left,
                right,
            };
            let (_, ev) = run(&mut p, t, 0.1, one);
            assert_eq!(ev, [Event::Catch], "{left} / {right}");
        }
    }

    /// 振り上げが弱いと、逃げられる。
    #[test]
    fn a_weak_swing_slips() {
        let mut p = pond();
        let t = held(&mut p);
        let (_, ev) = run(&mut p, t, 0.7, both(250.0));
        assert_eq!(ev, [Event::Slip]);
    }

    /// つかんだまま振り上げないと、しばらくで逃げる。
    #[test]
    fn holding_still_too_long_slips() {
        let mut p = pond();
        let t = held(&mut p);
        let still = Seen {
            grab: 0.5,
            ..Seen::default()
        };
        let (t, ev) = run(&mut p, t, 4.5, still);
        assert!(ev.is_empty());
        let (_, ev) = run(&mut p, t, 1.0, still);
        assert_eq!(ev, [Event::Slip]);
    }

    /// **逃げた直後はつかめない。** 手を入れたままでも、すぐまた捕まらない。
    #[test]
    fn a_slipped_fish_cannot_be_grabbed_right_away() {
        let mut p = pond();
        let t = held(&mut p);
        let (t, _) = run(&mut p, t, 0.7, both(250.0));
        let (t, ev) = run(&mut p, t, 1.2, both(0.0));
        assert!(ev.is_empty(), "逃げた直後につかまった: {ev:?}");
        let (_, ev) = run(&mut p, t, 2.0, both(0.0));
        assert!(ev.contains(&Event::Grab), "いつまでもつかめない");
    }

    /// **逃げるたびに釣れやすくなる。** 釣れたら元に戻る。
    #[test]
    fn slipping_makes_the_next_catch_easier() {
        let mut p = pond();
        let full = p.need();
        let mut t = held(&mut p);
        for _ in 0..3 {
            let weak = both(p.need() * 0.4);
            let (t2, ev) = run(&mut p, t, 0.7, weak);
            assert_eq!(ev, [Event::Slip]);
            let (t2, _) = run(&mut p, t2, 2.5, Seen::default());
            let touch = Seen {
                grab: 0.5,
                ..Seen::default()
            };
            let (t2, _) = run(&mut p, t2, 0.4, touch);
            t = t2;
        }
        let eased = p.need();
        assert!(eased < full * 0.7, "釣れやすくなっていない");
        // 元の速さには届かないが、下がった速さには届く振り上げで釣れる。
        let (_, ev) = run(&mut p, t, 0.2, both(eased + 10.0));
        assert_eq!(ev, [Event::Catch]);
        assert!(eased + 10.0 < full);
        assert_eq!(p.need(), full);
    }

    /// 出る割合は足して 100。どの種類もちゃんと出てくる。
    #[test]
    fn every_kind_shows_up() {
        assert_eq!(Kind::ALL.iter().map(|k| k.weight()).sum::<u32>(), 100);
        let mut p = pond();
        let kinds: Vec<Kind> = (0..400).map(|_| p.pick().kind).collect();
        for k in Kind::ALL {
            let n = kinds.iter().filter(|&&x| x == k).count();
            assert!(n > 20, "{k:?} が {n} 匹しか出ない");
        }
        let sharks = kinds.iter().filter(|&&x| x == Kind::Shark).count();
        let fish = kinds.iter().filter(|&&x| x == Kind::Fish).count();
        assert!(sharks < fish / 2, "サメが多すぎる");
    }

    /// 選んだ色は、その種類の色。
    #[test]
    fn colors_match_the_kind() {
        let mut p = pond();
        for _ in 0..200 {
            let c = p.pick();
            assert!(c.kind.colors().contains(&c.color), "{c:?}");
        }
    }

    /// 速さの下げ方には底がある。
    #[test]
    fn the_need_has_a_floor() {
        let mut p = pond();
        p.slips = 50;
        assert_eq!(p.need(), Config::default().swing_speed * FLOOR);
    }

    /// マウスで押した魚はすぐ釣れる。何もない所なら何も起きない。
    #[test]
    fn a_poke_catches_the_fish_under_it() {
        let mut p = pond();
        let (t, _) = run(&mut p, Duration::ZERO, 4.0, Seen::default());
        assert!(!p.poke(t, -999, -999));
        let f = p.fish()[0].clone();
        assert!(p.poke(t, f.x as i32, f.y as i32));
        assert!(matches!(p.fish()[0].state, State::Caught { .. }));
    }

    /// 振り上げを見る範囲は、魚の左と右に分かれ、魚の上まで伸びる。
    #[test]
    fn the_zones_split_left_and_right_and_reach_above() {
        let mut p = pond();
        run(&mut p, Duration::ZERO, 4.0, Seen::default());
        let f = &p.fish()[0];
        let (l, r) = f.zones();
        assert!(l.x + l.w <= r.x + 1);
        assert!(l.contains(f.x as i32 - 50, f.y as i32 - 200));
        assert!(r.contains(f.x as i32 + 50, f.y as i32 - 200));
        assert!(!l.contains(f.x as i32 + 50, f.y as i32));
    }
}
