//! どこが動いたか。前の絵との差を取るだけ。向きは見ない（`Flow` を見ること）。

use kit::{Image, HEIGHT, WIDTH};

use crate::grid::brightness;

/// 動きを見る升目の数。カメラの絵（320×180）の 4×4 画素で1升、
/// キャンバス（1280×720）では 16×16 画素で1升になる。
pub const COLS: usize = 80;
pub const ROWS: usize = 45;

/// **これより広く動いたら、無かったことにする。** 部屋の明かりが
/// 点いたり、カメラが露出を合わせ直したりすると全面が変わる。それで
/// 風船が全部割れると、何もしていないのに割れたことになる。
pub(crate) const GLOBAL: f32 = 0.5;

/// 前の絵を覚えておいて、差を出す。
#[derive(Default)]
pub struct Motion {
    prev: Option<Vec<u8>>,
}

/// 動いた升目。
pub struct Moved {
    cells: Vec<bool>,
}

impl Motion {
    /// 新しい絵を見て、前の絵から動いた升目を返す。**最初の1枚は
    /// 比べる相手が無いので、何も動いていない。**
    pub fn see(&mut self, img: &Image, threshold: u8) -> Moved {
        let now = brightness(img, COLS, ROWS);
        let cells = match &self.prev {
            Some(prev) if prev.len() == now.len() => prev
                .iter()
                .zip(&now)
                .map(|(a, b)| a.abs_diff(*b) >= threshold)
                .collect(),
            _ => vec![false; COLS * ROWS],
        };
        self.prev = Some(now);
        let moved = Moved { cells };
        if moved.ratio() > GLOBAL {
            return Moved::still();
        }
        moved
    }
}

impl Moved {
    /// 何も動いていない。
    pub fn still() -> Moved {
        Moved {
            cells: vec![false; COLS * ROWS],
        }
    }

    /// 全体のうち動いた升目の割合。
    pub fn ratio(&self) -> f32 {
        self.cells.iter().filter(|&&m| m).count() as f32 / self.cells.len() as f32
    }

    /// キャンバスの丸 `(x, y, r)` の中で、動いた升目の割合。
    /// 升目の真ん中が丸に入っているものだけ数える。丸が画面の外なら 0。
    pub fn ratio_in(&self, x: f32, y: f32, r: f32) -> f32 {
        let (cw, ch) = (WIDTH as f32 / COLS as f32, HEIGHT as f32 / ROWS as f32);
        let (mut inside, mut moved) = (0usize, 0usize);
        let col = |v: f32| ((v / cw).floor().max(0.0) as usize).min(COLS);
        let row = |v: f32| ((v / ch).floor().max(0.0) as usize).min(ROWS);
        for gy in row(y - r)..row(y + r + ch) {
            for gx in col(x - r)..col(x + r + cw) {
                if gx >= COLS || gy >= ROWS {
                    continue;
                }
                let (cx, cy) = ((gx as f32 + 0.5) * cw, (gy as f32 + 0.5) * ch);
                if (cx - x).powi(2) + (cy - y).powi(2) > r * r {
                    continue;
                }
                inside += 1;
                moved += self.cells[gy * COLS + gx] as usize;
            }
        }
        if inside == 0 {
            0.0
        } else {
            moved as f32 / inside as f32
        }
    }

    /// 動いた升目の真ん中（キャンバスの座標）。画面に光を出すのに使う。
    pub fn spots(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let (cw, ch) = (WIDTH / COLS as i32, HEIGHT / ROWS as i32);
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, &m)| m)
            .map(move |(i, _)| {
                let (gx, gy) = ((i % COLS) as i32, (i / COLS) as i32);
                (gx * cw + cw / 2, gy * ch + ch / 2)
            })
    }
}

#[cfg(test)]
mod tests {
    use kit::{CAMERA_HEIGHT, CAMERA_WIDTH};

    use super::*;
    use crate::sample::{blob, grey};

    #[test]
    fn the_first_frame_moves_nothing() {
        let mut m = Motion::default();
        assert_eq!(m.see(&blob(640, 360, 100), 28).ratio(), 0.0);
    }

    #[test]
    fn the_same_frame_twice_moves_nothing() {
        let mut m = Motion::default();
        m.see(&grey(), 28);
        assert_eq!(m.see(&grey(), 28).ratio(), 0.0);
    }

    /// 白くなった所だけが動いて見える。
    #[test]
    fn only_the_changed_place_moves() {
        let mut m = Motion::default();
        m.see(&grey(), 28);
        let moved = m.see(&blob(300, 400, 80), 28);
        assert!(moved.ratio_in(300.0, 400.0, 60.0) > 0.9);
        assert_eq!(moved.ratio_in(1000.0, 200.0, 80.0), 0.0);
    }

    /// **わずかな揺らぎでは動かない。** カメラのざらつきで割れ続けない。
    #[test]
    fn a_small_flicker_is_not_motion() {
        let mut m = Motion::default();
        m.see(&grey(), 28);
        let dim = Image::filled(CAMERA_WIDTH, CAMERA_HEIGHT, (90, 90, 90));
        assert_eq!(m.see(&dim, 28).ratio(), 0.0);
    }

    /// **全面が変わったら、無かったことにする。** 明かりが点いただけで
    /// 風船が全部割れないように。
    #[test]
    fn a_change_of_the_whole_picture_is_ignored() {
        let mut m = Motion::default();
        m.see(&grey(), 28);
        let bright = Image::filled(CAMERA_WIDTH, CAMERA_HEIGHT, (220, 220, 220));
        assert_eq!(m.see(&bright, 28).ratio(), 0.0);
    }

    /// 画面の外の丸は数えない。落ちもしない。
    #[test]
    fn a_circle_outside_the_screen_is_zero() {
        let moved = Moved {
            cells: vec![true; COLS * ROWS],
        };
        assert_eq!(moved.ratio_in(-500.0, -500.0, 50.0), 0.0);
        assert_eq!(moved.ratio_in(5000.0, 5000.0, 50.0), 0.0);
        assert!(moved.ratio_in(1270.0, 710.0, 30.0) > 0.0);
    }
}
