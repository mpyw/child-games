//! 風船の出し方と割れ方。
//!
//! **時刻は外から受け取る。** 中で `Instant::now` を見ると、出す・浮かぶ・
//! 割れるの順を決まった形で確かめられない。

use std::time::Duration;

use kit::{Rgb, HEIGHT, WIDTH};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use vision::Moved;

use crate::config::Config;

/// 風船の色。離れたテレビでも見分けられる、はっきりした色だけ。
pub const COLORS: [Rgb; 8] = [
    (230, 57, 70),
    (244, 140, 6),
    (255, 214, 10),
    (82, 183, 136),
    (72, 202, 228),
    (58, 110, 220),
    (157, 78, 221),
    (255, 112, 166),
];

/// 風船の大きさ（キャンバスの画素）。**大きく取る。** 2歳児の手は
/// 狙った所に行かない。
const RADIUS: f32 = 88.0;
/// 子どもの手が届く帯。画面の高さに対する割合で、上端と下端。
///
/// **風船はここをゆっくり通る。** 下から上がってきて、帯の中では
/// 漂うように遅くなり、抜けたら上へ飛んでいく。止めてしまうと風船らしく
/// なく、同じ速さで抜けると触る前に飛んでいく。
pub const REACH: (f32, f32) = (0.32, 0.68);
/// 帯より下を上がる速さ（毎秒の画素）。早く手の届く所まで来させる。
const RISE: f32 = 170.0;
/// 帯の中の速さ。帯（約 260 画素）を 4.5 秒ほどかけて通る。
const LINGER: f32 = 58.0;
/// 帯を抜けたあとの速さ。飛んでいく、と分かる速さ。
const DRIFT: f32 = 120.0;
/// 速さを切り替える幅（画素）。急に変わると、引っかかったように見える。
const EASE: f32 = 80.0;
/// 現れてから触れるようになるまで。出てきた瞬間に、たまたまそこで
/// 動いていた体で割れると、割った手応えが無い。
const ARM: Duration = Duration::from_millis(400);
/// 次の風船を出すまでの間。いっぺんに出すと、どれを狙うかで迷う。
const STAGGER: Duration = Duration::from_millis(700);
/// 割れたあと、次を出すまでの間。**叩いても何も無い時間を作らない。**
const REFILL: Duration = Duration::from_millis(300);
/// これだけ割れなければ、風船を大きくして誘う。失敗にはしない。
const IDLE: Duration = Duration::from_secs(8);
/// 誘うときの大きさの上限（倍）。
const GROW: f32 = 1.35;
/// 割れた絵を残す長さ。
pub const BURST: Duration = Duration::from_millis(500);

/// 漂っている風船。
#[derive(Debug, Clone)]
pub struct Balloon {
    pub x: f32,
    pub y: f32,
    pub color: Rgb,
    born: Duration,
    /// 揺れ方をずらす。全部が揃って揺れると作り物に見える。
    phase: f32,
}

/// 割れた跡。しばらく弾ける絵を出す。
#[derive(Debug, Clone)]
pub struct Burst {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub color: Rgb,
    pub at: Duration,
}

pub struct Field {
    balloons: Vec<Balloon>,
    bursts: Vec<Burst>,
    popped: Vec<Rgb>,
    next_spawn: Duration,
    last_pop: Duration,
    max: usize,
    touch: f32,
    rng: StdRng,
}

impl Field {
    pub fn new(cfg: &Config, seed: u64) -> Self {
        Self {
            balloons: Vec::new(),
            bursts: Vec::new(),
            popped: Vec::new(),
            next_spawn: Duration::ZERO,
            last_pop: Duration::ZERO,
            max: cfg.max_balloons,
            touch: cfg.touch_ratio,
            rng: StdRng::seed_from_u64(seed),
        }
    }

    pub fn balloons(&self) -> &[Balloon] {
        &self.balloons
    }

    pub fn bursts(&self) -> &[Burst] {
        &self.bursts
    }

    /// 割った風船の色。割った順。
    pub fn popped(&self) -> &[Rgb] {
        &self.popped
    }

    /// 今の大きさ。**しばらく割れなければ大きくなる。**
    pub fn radius(&self, now: Duration) -> f32 {
        let idle = now.saturating_sub(self.last_pop).saturating_sub(IDLE);
        let t = (idle.as_secs_f32() / 6.0).min(1.0);
        RADIUS * (1.0 + (GROW - 1.0) * t)
    }

    /// 時刻を進める。浮かべて、揺らして、抜けたものを消し、足りなければ足す。
    pub fn step(&mut self, now: Duration, dt: f32) {
        // しばらく割れないと、浮かぶのも揺れるのも遅くする。追いかけやすいように。
        let calm = if now.saturating_sub(self.last_pop) > IDLE {
            0.5
        } else {
            1.0
        };
        for b in &mut self.balloons {
            b.y -= speed(b.y) * calm * dt;
            b.x += (now.as_secs_f32() * 0.9 + b.phase).cos() * 18.0 * calm * dt;
        }
        // **上から抜けた風船は黙って消す。** 失敗としては見せない。空いたぶんは
        // 下からすぐ次が出る。紐まで抜けきってから消す。
        let gone = -RADIUS * GROW * 2.6;
        self.balloons.retain(|b| b.y > gone);
        self.bursts.retain(|p| now.saturating_sub(p.at) < BURST);

        if self.balloons.len() < self.max && now >= self.next_spawn {
            self.spawn(now);
            self.next_spawn = now + STAGGER;
        }
    }

    /// 下の端の外から1つ出す。**他の風船と重ならない所を選ぶ。**
    fn spawn(&mut self, now: Duration) {
        let r = RADIUS;
        let (lo, hi) = (WIDTH as f32 * 0.12, WIDTH as f32 * 0.88);
        let mut x = self.rng.gen_range(lo..hi);
        for _ in 0..12 {
            if self.balloons.iter().all(|b| (b.x - x).abs() > r * 2.3) {
                break;
            }
            x = self.rng.gen_range(lo..hi);
        }
        let color = COLORS[self.rng.gen_range(0..COLORS.len())];
        self.balloons.push(Balloon {
            x,
            y: HEIGHT as f32 + r * 1.2,
            color,
            born: now,
            phase: self.rng.gen_range(0.0..std::f32::consts::TAU),
        });
    }

    /// 触れる風船か。画面の中まで上がってきて、少し経ったもの。
    fn armed(&self, b: &Balloon, now: Duration) -> bool {
        now.saturating_sub(b.born) >= ARM && b.y + self.radius(now) * 0.5 < HEIGHT as f32
    }

    /// 動いた所に重なった風船を割る。割った数を返す。
    ///
    /// **見る丸は風船より一回り大きい。** 2歳児の手は風船の縁をかすめる
    /// くらいで止まる。そこで割れないと、届いたのに無視されたことになる。
    pub fn touch(&mut self, now: Duration, moved: &Moved) -> usize {
        let r = self.radius(now) * 1.2;
        let hit: Vec<usize> = (0..self.balloons.len())
            .filter(|&i| {
                let b = &self.balloons[i];
                self.armed(b, now) && moved.ratio_in(b.x, b.y, r) >= self.touch
            })
            .collect();
        for &i in hit.iter().rev() {
            self.pop(i, now);
        }
        hit.len()
    }

    /// 画面のこの位置を押した（マウス）。**甘めに当てる。** 風船より少し
    /// 広い範囲で一番近いものを割る。カメラが無いときに試すための口。
    pub fn poke(&mut self, now: Duration, x: i32, y: i32) -> bool {
        let reach = self.radius(now) * 1.3;
        let (x, y) = (x as f32, y as f32);
        let nearest = self
            .balloons
            .iter()
            .enumerate()
            .map(|(i, b)| (i, (b.x - x).hypot(b.y - y)))
            .filter(|&(_, d)| d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match nearest {
            Some((i, _)) => {
                self.pop(i, now);
                true
            }
            None => false,
        }
    }

    fn pop(&mut self, i: usize, now: Duration) {
        let r = self.radius(now);
        let b = self.balloons.remove(i);
        self.bursts.push(Burst {
            x: b.x,
            y: b.y,
            r,
            color: b.color,
            at: now,
        });
        self.popped.push(b.color);
        self.last_pop = now;
        // 割れたらすぐ次を出す。
        self.next_spawn = self.next_spawn.min(now + REFILL);
    }
}

/// 高さ `y` での浮かぶ速さ。帯の下では速く、帯の中では遅く、抜けたら
/// また少し速く。境目は `EASE` の幅でなだらかに繋ぐ。
fn speed(y: f32) -> f32 {
    let (top, bottom) = (HEIGHT as f32 * REACH.0, HEIGHT as f32 * REACH.1);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t.clamp(0.0, 1.0);
    if y > bottom {
        lerp(LINGER, RISE, (y - bottom) / EASE)
    } else if y > top {
        LINGER
    } else {
        lerp(LINGER, DRIFT, (top - y) / EASE)
    }
}

#[cfg(test)]
mod tests {
    use vision::sample::{blob, grey};
    use vision::Motion;

    use super::*;

    const FRAME: f32 = 1.0 / 30.0;

    /// `secs` 秒ぶん、30fps で進める。
    fn run(field: &mut Field, from: Duration, secs: f32) -> Duration {
        let mut t = from;
        for _ in 0..(secs / FRAME) as usize {
            t += Duration::from_secs_f32(FRAME);
            field.step(t, FRAME);
        }
        t
    }

    fn field() -> Field {
        Field::new(&Config::default(), 7)
    }

    /// 決めた数までしか出さない。出し始めは1つずつ。
    #[test]
    fn balloons_come_one_by_one_up_to_the_limit() {
        let mut f = field();
        f.step(Duration::ZERO, 0.0);
        assert_eq!(f.balloons().len(), 1, "最初の1つが出ていない");
        let t = run(&mut f, Duration::ZERO, 0.5);
        assert_eq!(f.balloons().len(), 1, "いっぺんに出した");
        run(&mut f, t, 10.0);
        assert_eq!(f.balloons().len(), Config::default().max_balloons);
    }

    /// **手の届く帯はゆっくり通り、抜けたら上へ飛んでいく。** 飛んでいった
    /// ぶんは下から次が出る。
    #[test]
    fn balloons_linger_within_reach_then_fly_away() {
        let mut f = Field::new(
            &Config {
                max_balloons: 1,
                ..Config::default()
            },
            5,
        );
        f.step(Duration::ZERO, 0.0);
        let (top, bottom) = (HEIGHT as f32 * REACH.0, HEIGHT as f32 * REACH.1);
        let (mut t, mut in_reach, mut below) = (Duration::ZERO, 0.0f32, 0.0f32);
        while f.balloons()[0].y > -RADIUS {
            t = run(&mut f, t, FRAME * 1.5);
            let b = &f.balloons()[0];
            assert!(b.x > 0.0 && b.x < WIDTH as f32, "{b:?}");
            if (top..bottom).contains(&b.y) {
                in_reach += FRAME;
            } else if b.y > bottom {
                below += FRAME;
            }
            assert!(t < Duration::from_secs(30), "上へ抜けていかない");
        }
        assert!(in_reach >= 4.0, "帯の中を {in_reach} 秒で通り過ぎた");
        assert!(below < 3.0, "帯まで上がるのが遅い（{below} 秒）");

        // 抜けきったら消えて、次が下から出る。
        run(&mut f, t, 5.0);
        let b = &f.balloons()[0];
        assert!(b.y > top, "次の風船が出ていない: {b:?}");
    }

    /// 速さは境目でなだらかに変わる。急に変わると引っかかったように見える。
    #[test]
    fn the_speed_changes_smoothly() {
        let mut y = HEIGHT as f32 * 1.5;
        let mut prev = speed(y);
        while y > -RADIUS * 3.0 {
            y -= 1.0;
            let s = speed(y);
            assert!((s - prev).abs() < 2.0, "{y} で {prev} → {s}");
            prev = s;
        }
    }

    /// 触った所の風船が割れる。離れた所を触っても割れない。
    #[test]
    fn a_balloon_pops_where_it_is_touched() {
        let mut f = Field::new(
            &Config {
                max_balloons: 1,
                ..Config::default()
            },
            7,
        );
        let t = run(&mut f, Duration::ZERO, 4.0);
        let b = f.balloons()[0].clone();
        let mut m = Motion::default();
        m.see(&grey(), 28);
        // 離れた所を触っても割れない。
        let away = m.see(&blob((b.x as i32 + 600) % WIDTH, b.y as i32, 90), 28);
        assert_eq!(f.touch(t, &away), 0, "触っていないのに割れた");
        m.see(&grey(), 28);
        let on = m.see(&blob(b.x as i32, b.y as i32, 90), 28);
        let before = f.balloons().len();
        assert_eq!(f.touch(t, &on), 1);
        assert_eq!(f.balloons().len(), before - 1);
        assert_eq!(f.popped(), &[b.color]);
        assert_eq!(f.bursts().len(), 1);
    }

    /// **割れたらすぐ次が出る。** 叩いても何も無い時間を作らない。
    #[test]
    fn a_pop_brings_the_next_balloon_soon() {
        let mut f = Field::new(
            &Config {
                max_balloons: 1,
                ..Config::default()
            },
            3,
        );
        let t = run(&mut f, Duration::ZERO, 4.0);
        let b = f.balloons()[0].clone();
        assert!(f.poke(t, b.x as i32, b.y as i32));
        assert!(f.balloons().is_empty());
        run(&mut f, t, 0.5);
        assert_eq!(f.balloons().len(), 1, "次が出ていない");
    }

    /// マウスは甘めに当てる。遠すぎれば割れない。
    #[test]
    fn a_poke_is_forgiving_but_not_everywhere() {
        let mut f = field();
        let t = run(&mut f, Duration::ZERO, 4.0);
        let b = f.balloons()[0].clone();
        assert!(!f.poke(t, -1000, -1000));
        assert!(f.poke(t, (b.x + RADIUS * 1.1) as i32, b.y as i32));
    }

    /// **しばらく割れなければ大きくなって誘う。** 割れたら元に戻る。
    #[test]
    fn idle_balloons_grow_to_invite() {
        let mut f = field();
        let t = run(&mut f, Duration::ZERO, 4.0);
        assert_eq!(f.radius(t), RADIUS);
        let t = run(&mut f, t, 20.0);
        assert!(f.radius(t) > RADIUS * 1.3);
        let b = f.balloons()[0].clone();
        assert!(f.poke(t, b.x as i32, b.y as i32));
        assert_eq!(f.radius(t), RADIUS);
    }

    /// 割れた跡はしばらくで消える。
    #[test]
    fn bursts_fade() {
        let mut f = field();
        let t = run(&mut f, Duration::ZERO, 4.0);
        let b = f.balloons()[0].clone();
        f.poke(t, b.x as i32, b.y as i32);
        run(&mut f, t, 1.0);
        assert!(f.bursts().is_empty());
    }
}
