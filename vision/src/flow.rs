//! どこが・どちらへ・どれだけ速く動いたか。
//!
//! **升目ごとに、前の絵のどこから来たかを探す**（ブロックマッチング）。
//! 今の絵の升目のまわり 3×3 を、前の絵の上下左右に少しずつずらした位置と
//! 比べ、一番よく合ったずれをその升目の動きとする。動いた升目だけで探すので、
//! 止まっている所には手間を掛けない。
//!
//! `Motion` は「変わったか」しか分からない。振り上げたのか振り下ろしたのかを
//! 見分けたい遊びはこちらを使う。

use kit::{Image, Rect, WIDTH};

use crate::grid::brightness;
use crate::motion::GLOBAL;

/// 向きを見る升目の数。カメラの絵（320×180）の 2×2 画素で1升、
/// キャンバス（1280×720）では 8×8 画素で1升になる。
///
/// **`Motion` より細かくする。** 16 画素の升目だと、ゆっくり持ち上げた手が
/// 1枚の間に1升も進まず、ずれが 0 に丸まる。
pub const COLS: usize = 160;
pub const ROWS: usize = 90;

/// 1升のキャンバスでの大きさ。
const CELL: f32 = WIDTH as f32 / COLS as f32;

/// 1枚あたり探すずれの幅（升）。8 画素 × 6 = 48 画素。毎秒30枚なら、
/// 毎秒 1440 画素までの速さを測れる。2歳児の振り上げはこれに収まる。
const REACH: i32 = 6;

/// **ずらして良くなった度合いがこれ未満なら、向きは分からないとする。**
/// 一面同じ色の所は、どこへずらしても同じように合う。そこで決めた向きは
/// 当てずっぽうになる。ずらさないときより 4 割以上よく合ったものだけ残す。
const CONFIDENT: f32 = 0.6;

/// 前の絵を覚えておいて、動きを出す。
#[derive(Default)]
pub struct Flow {
    prev: Option<Vec<u8>>,
}

/// 動いた升目と、その向き。
pub struct Flowed {
    moved: Vec<bool>,
    /// キャンバスの画素で、1枚あたりのずれ。`y` は下が正。分からなければ `None`。
    vectors: Vec<Option<(f32, f32)>>,
}

/// 範囲の中の動きのまとめ。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Region {
    /// 範囲の中で動いた升目の割合。
    pub moving: f32,
    /// 向きが分かった升目の、上向きの速さの平均（キャンバスの画素 / 1枚）。
    /// **上が正。** 下へ動けば負になる。
    pub up: f32,
    /// 向きが分かった升目の数。少なければ `up` は当てにならない。
    pub vectors: usize,
}

impl Flow {
    /// 新しい絵を見て、前の絵からの動きを返す。**最初の1枚は比べる相手が
    /// 無いので、何も動いていない。** 全面が変わったときも同じ（`Motion` と同じ）。
    pub fn see(&mut self, img: &Image, threshold: u8) -> Flowed {
        let now = brightness(img, COLS, ROWS);
        let prev = match self.prev.replace(now.clone()) {
            Some(p) if p.len() == now.len() => p,
            _ => return Flowed::still(),
        };
        let moved: Vec<bool> = prev
            .iter()
            .zip(&now)
            .map(|(a, b)| a.abs_diff(*b) >= threshold)
            .collect();
        let count = moved.iter().filter(|&&m| m).count();
        if count as f32 / moved.len() as f32 > GLOBAL {
            return Flowed::still();
        }
        let vectors = (0..COLS * ROWS)
            .map(|i| {
                moved[i]
                    .then(|| track(&prev, &now, (i % COLS) as i32, (i / COLS) as i32))
                    .flatten()
            })
            .collect();
        Flowed { moved, vectors }
    }
}

impl Flowed {
    /// 何も動いていない。
    pub fn still() -> Flowed {
        Flowed {
            moved: vec![false; COLS * ROWS],
            vectors: vec![None; COLS * ROWS],
        }
    }

    /// キャンバスの丸 `(x, y, r)` の中で、動いた升目の割合。丸が画面の外なら 0。
    pub fn ratio_in(&self, x: f32, y: f32, r: f32) -> f32 {
        let (mut inside, mut moved) = (0usize, 0usize);
        for (i, (cx, cy)) in self.cells_in(x - r, y - r, x + r, y + r) {
            if (cx - x).powi(2) + (cy - y).powi(2) > r * r {
                continue;
            }
            inside += 1;
            moved += self.moved[i] as usize;
        }
        if inside == 0 {
            0.0
        } else {
            moved as f32 / inside as f32
        }
    }

    /// 長方形の中の動きをまとめる。
    pub fn region(&self, area: Rect) -> Region {
        let (x0, y0) = (area.x as f32, area.y as f32);
        let (x1, y1) = (x0 + area.w as f32, y0 + area.h as f32);
        let (mut inside, mut moved, mut up, mut vectors) = (0usize, 0usize, 0.0f32, 0usize);
        for (i, _) in self.cells_in(x0, y0, x1, y1) {
            inside += 1;
            moved += self.moved[i] as usize;
            if let Some((_, vy)) = self.vectors[i] {
                up -= vy;
                vectors += 1;
            }
        }
        Region {
            moving: if inside == 0 {
                0.0
            } else {
                moved as f32 / inside as f32
            },
            up: if vectors == 0 {
                0.0
            } else {
                up / vectors as f32
            },
            vectors,
        }
    }

    /// 向きが分かった升目。`(真ん中の x, y, ずれの x, y)`、キャンバスの画素。
    /// 調整のために画面へ出すのに使う。
    pub fn vectors(&self) -> impl Iterator<Item = (i32, i32, f32, f32)> + '_ {
        self.vectors.iter().enumerate().filter_map(|(i, v)| {
            let (vx, vy) = (*v)?;
            let (cx, cy) = center(i);
            Some((cx as i32, cy as i32, vx, vy))
        })
    }

    /// 真ん中が `[x0, x1) × [y0, y1)` に入る升目と、その真ん中。
    fn cells_in(
        &self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    ) -> impl Iterator<Item = (usize, (f32, f32))> + '_ {
        let col = |v: f32| ((v / CELL).floor().max(0.0) as usize).min(COLS);
        let row = |v: f32| ((v / CELL).floor().max(0.0) as usize).min(ROWS);
        let (c0, c1, r0, r1) = (col(x0), col(x1 + CELL), row(y0), row(y1 + CELL));
        (r0..r1)
            .flat_map(move |gy| (c0..c1).map(move |gx| gy * COLS + gx))
            .map(|i| (i, center(i)))
            .filter(move |&(_, (cx, cy))| cx >= x0 && cx < x1 && cy >= y0 && cy < y1)
    }
}

fn center(i: usize) -> (f32, f32) {
    let (gx, gy) = ((i % COLS) as f32, (i / COLS) as f32);
    ((gx + 0.5) * CELL, (gy + 0.5) * CELL)
}

/// 升目 `(x, y)` が前の絵のどこから来たか。キャンバスの画素で返す。
///
/// 一番よく合ったずれのまわりで放物線を当てて、1升より細かく詰める。
/// 升目単位のままだと、ゆっくりした動きが 0 か 8 画素かの二択になる。
fn track(prev: &[u8], now: &[u8], x: i32, y: i32) -> Option<(f32, f32)> {
    let cost = |dx: i32, dy: i32| sad(prev, now, x, y, dx, dy);
    let still = cost(0, 0)?;
    // **同じくらい合うなら、ずれの小さいほうを採る。** 縁が直線に近い所は、
    // 縁に沿ってずらしてもほとんど同じに合う。探す順の先頭（上）を採って
    // いた頃は、横に動いただけで上向きに偏った。
    let far = |dx: i32, dy: i32| dx * dx + dy * dy;
    let mut best = (0, 0, still);
    for dy in -REACH..=REACH {
        for dx in -REACH..=REACH {
            if let Some(c) = cost(dx, dy) {
                let tie = (c - best.2).abs() < 0.5;
                if (c < best.2 && !tie) || (tie && far(dx, dy) < far(best.0, best.1)) {
                    best = (dx, dy, c);
                }
            }
        }
    }
    let (bx, by, c0) = best;
    if (bx, by) == (0, 0) || c0 > still * CONFIDENT {
        return None;
    }
    let refine = |minus: Option<f32>, plus: Option<f32>| match (minus, plus) {
        (Some(m), Some(p)) => {
            let curve = m - 2.0 * c0 + p;
            if curve > 0.0 {
                ((m - p) / (2.0 * curve)).clamp(-0.5, 0.5)
            } else {
                0.0
            }
        }
        _ => 0.0,
    };
    let ox = refine(cost(bx - 1, by), cost(bx + 1, by));
    let oy = refine(cost(bx, by - 1), cost(bx, by + 1));
    Some(((bx as f32 + ox) * CELL, (by as f32 + oy) * CELL))
}

/// 今の絵の `(x, y)` のまわり 3×3 と、前の絵の `(x - dx, y - dy)` のまわりの
/// 差の絶対値の平均。はみ出す升は数えない。比べられる升が半分未満なら `None`。
fn sad(prev: &[u8], now: &[u8], x: i32, y: i32, dx: i32, dy: i32) -> Option<f32> {
    let (mut sum, mut n) = (0u32, 0u32);
    for j in -1..=1 {
        for i in -1..=1 {
            let (cx, cy) = (x + i, y + j);
            let (px, py) = (cx - dx, cy - dy);
            let inside = |a: i32, b: i32| a >= 0 && b >= 0 && a < COLS as i32 && b < ROWS as i32;
            if !inside(cx, cy) || !inside(px, py) {
                continue;
            }
            let a = now[cy as usize * COLS + cx as usize];
            let b = prev[py as usize * COLS + px as usize];
            sum += a.abs_diff(b) as u32;
            n += 1;
        }
    }
    (n >= 5).then(|| sum as f32 / n as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::{blob, blobs, grey};

    const AROUND: Rect = Rect {
        x: 440,
        y: 200,
        w: 400,
        h: 400,
    };

    /// 1枚目を見せてから2枚目を見せる。
    fn between(a: &Image, b: &Image) -> Flowed {
        let mut f = Flow::default();
        f.see(a, 28);
        f.see(b, 28)
    }

    /// **上へ動いたら上向き、下へ動いたら下向き。** 速さもだいたい合う。
    #[test]
    fn upward_and_downward_are_told_apart() {
        let up = between(&blob(640, 420, 70), &blob(640, 390, 70)).region(AROUND);
        assert!(up.vectors > 10, "向きが分かった升目が少ない: {up:?}");
        assert!((20.0..40.0).contains(&up.up), "上へ 30 画素のはず: {up:?}");

        let down = between(&blob(640, 390, 70), &blob(640, 420, 70)).region(AROUND);
        assert!(down.up < -20.0, "下へ動いたのに上向きに見えた: {down:?}");
    }

    /// 横へ動いただけなら、上向きの速さはほぼ 0。
    #[test]
    fn sideways_is_not_up() {
        let side = between(&blob(600, 400, 70), &blob(630, 400, 70)).region(AROUND);
        assert!(side.vectors > 10, "{side:?}");
        assert!(
            side.up.abs() < 6.0,
            "横に動いただけで上下に見えた: {side:?}"
        );
    }

    /// ゆっくりした動きも 0 に丸めない。1升（8 画素）より小さくても測れる。
    #[test]
    fn slow_motion_is_not_rounded_away() {
        let slow = between(&blob(640, 410, 70), &blob(640, 404, 70)).region(AROUND);
        assert!(slow.up > 2.0 && slow.up < 12.0, "{slow:?}");
    }

    /// 左右に分けて見られる。片方だけ動けば、もう片方は動いていない。
    #[test]
    fn left_and_right_are_separate() {
        let left = Rect {
            x: 200,
            y: 200,
            w: 300,
            h: 400,
        };
        let right = Rect { x: 780, ..left };
        let f = between(
            &blobs(&[(350, 420, 60), (930, 420, 60)]),
            &blobs(&[(350, 390, 60), (930, 420, 60)]),
        );
        assert!(f.region(left).up > 15.0, "{:?}", f.region(left));
        assert_eq!(f.region(right).vectors, 0, "{:?}", f.region(right));
    }

    #[test]
    fn nothing_moves_on_the_first_frame_or_a_still_picture() {
        let mut f = Flow::default();
        assert_eq!(f.see(&grey(), 28).region(Rect::SCREEN).vectors, 0);
        assert_eq!(f.see(&grey(), 28).region(Rect::SCREEN), Region::default());
    }

    /// 全面が変わったら、無かったことにする。
    #[test]
    fn a_change_of_the_whole_picture_is_ignored() {
        let bright = Image::filled(kit::CAMERA_WIDTH, kit::CAMERA_HEIGHT, (220, 220, 220));
        let f = between(&grey(), &bright);
        assert_eq!(f.region(Rect::SCREEN).moving, 0.0);
    }

    /// 動いた割合は丸の中で数える。画面の外なら 0。
    #[test]
    fn ratio_in_counts_inside_the_circle() {
        let f = between(&grey(), &blob(640, 400, 80));
        assert!(f.ratio_in(640.0, 400.0, 50.0) > 0.9);
        assert_eq!(f.ratio_in(100.0, 100.0, 50.0), 0.0);
        assert_eq!(f.ratio_in(-500.0, -500.0, 50.0), 0.0);
    }
}
