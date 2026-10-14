//! 何を映すか。鏡に映った自分、池、魚。
//!
//! 画素は知らない。図形を並べて `kit::Scene` にするところまでで、塗るのは
//! `home` の描き手。池は鏡の絵の下半分を青く染めて作る。

use std::sync::OnceLock;
use std::time::Duration;

use kit::{Image, Rect, Rgb, Scene, Shape, HEIGHT, WIDTH};
use vision::Flowed;

use crate::pond::{Creature, Fish, Kind, Pond, State, BODY, FLIGHT, HOP, WATER};

/// 鏡の暗さ。水の上の部分。**魚が主役。** 明るいままだと魚が紛れる。
const MIRROR: f32 = 0.6;
/// 池の色と、混ぜる割合。透けて自分の手が見えるくらいに留める。
const POND: Rgb = (30, 100, 200);
const TINT: f32 = 0.5;
/// 釣った魚を入れるバケツの位置。右上。
const BUCKET: (f32, f32) = (WIDTH as f32 - 120.0, 110.0);

/// 遊んでいる間の場面。`motion` が `Some` なら、動きの向きと強さも出す。
pub fn playing(mirror: &Image, pond: &Pond, now: Duration, motion: Option<&Flowed>) -> Scene {
    let mut shapes = waves(now);
    shapes.extend(bucket(pond.caught(), now));
    for f in pond.fish() {
        shapes.extend(fish_in_pond(f, now));
    }
    if let Some(flow) = motion {
        shapes.extend(debug(flow, pond));
    }
    Scene {
        image: Some(pond_over(mirror)),
        shapes,
        caption: format!(
            "── さかな {}ひき / つれた {}ひき / いる はやさ {:.0} ──",
            pond.fish().len(),
            pond.caught().len(),
            pond.need()
        ),
    }
}

/// 点数に使う字。`Game::glyphs` で `home` に渡し、フォントに入っているかを見させる。
pub const GLYPHS: &str = "0123456789ひき つれたね！";

/// 点数の上限。1分で釣れるのはせいぜい数十なので、ここで頭打ちにする。
const MAX_SCORE: usize = 999;

/// 釣った数を書いた字。「5ひき つれたね！」。
///
/// **一度だけ作って持っておく。** `Shape::Text` は `&'static str` しか
/// 持てない。毎フレーム作って手放せない文字列にすると、遊ぶほど漏れる。
pub fn score(n: usize) -> &'static str {
    static TEXTS: OnceLock<Vec<String>> = OnceLock::new();
    let texts = TEXTS.get_or_init(|| {
        (0..=MAX_SCORE)
            .map(|n| format!("{n}ひき つれたね！"))
            .collect()
    });
    &texts[n.min(MAX_SCORE)]
}

/// 曲が終わったあと。**上に釣った数を出し、下に釣った魚を並べて祝う。**
///
/// 1匹も釣れなかったときも、色を全部並べて同じように祝う。2歳児に
/// 「できなかった」を見せる理由は無い。
pub fn finale(caught: &[Creature], now: Duration) -> Scene {
    let shown: Vec<Creature> = if caught.is_empty() {
        Creature::parade()
    } else {
        caught.to_vec()
    };
    let top = HEIGHT * 28 / 100;
    let mut shapes = vec![Shape::Text {
        x: WIDTH / 2,
        y: HEIGHT * 14 / 100,
        px: HEIGHT / 8,
        text: score(caught.len()),
        fill: (250, 250, 250),
    }];
    shapes.extend(lineup(
        Rect {
            x: 0,
            y: top,
            w: WIDTH,
            h: HEIGHT - top,
        },
        &shown,
        now,
    ));
    Scene {
        shapes,
        caption: format!("── {} ──", score(caught.len())),
        ..Scene::default()
    }
}

/// メニューの札に描く絵。水と、魚とサメ。
pub fn icon(area: Rect) -> Vec<Shape> {
    let water = Rect {
        x: area.x,
        y: area.y + area.h / 2,
        w: area.w,
        h: area.h / 2,
    };
    let s = area.w as f32 / BODY.0 / 3.2;
    let mut shapes = vec![Shape::RoundRect {
        rect: water,
        r: area.h / 12,
        fill: POND,
    }];
    let fish = Creature {
        kind: Kind::Fish,
        color: crate::pond::COLORS[0],
    };
    let shark = Creature {
        kind: Kind::Shark,
        color: (120, 140, 165),
    };
    shapes.extend(creature(
        fish,
        area.x as f32 + area.w as f32 * 0.3,
        area.y as f32 + area.h as f32 * 0.42,
        s,
        1.0,
        0.0,
    ));
    shapes.extend(creature(
        shark,
        area.x as f32 + area.w as f32 * 0.68,
        area.y as f32 + area.h as f32 * 0.74,
        s,
        -1.0,
        0.0,
    ));
    shapes
}

/// 鏡の絵の下半分を青く染めて池にする。上は少し暗くする。
fn pond_over(mirror: &Image) -> Image {
    let line = (mirror.height as f32 * WATER / HEIGHT as f32) as usize;
    let pixels = mirror
        .pixels
        .iter()
        .enumerate()
        .map(|(i, &(r, g, b))| {
            if i / mirror.width < line {
                let m = |v: u8| (v as f32 * MIRROR) as u8;
                (m(r), m(g), m(b))
            } else {
                let m = |v: u8, w: u8| (v as f32 * MIRROR * (1.0 - TINT) + w as f32 * TINT) as u8;
                (m(r, POND.0), m(g, POND.1), m(b, POND.2))
            }
        })
        .collect();
    Image {
        width: mirror.width,
        height: mirror.height,
        pixels,
    }
}

/// 水面の波。細い帯を横に流す。
fn waves(now: Duration) -> Vec<Shape> {
    let t = now.as_secs_f32();
    (0..6)
        .map(|i| {
            let w = 140;
            let x = ((i as f32 * 240.0 + t * 40.0) % (WIDTH as f32 + w as f32)) as i32 - w;
            let y = WATER as i32 + ((t * 1.5 + i as f32).sin() * 6.0) as i32;
            Shape::RoundRect {
                rect: Rect { x, y, w, h: 8 },
                r: 4,
                fill: (170, 220, 255),
            }
        })
        .collect()
}

/// 池にいる魚1匹。様子で動かし方を変える。
fn fish_in_pond(f: &Fish, now: Duration) -> Vec<Shape> {
    let t = now.as_secs_f32();
    let bob = (t * 2.0 + f.phase).sin() * 6.0;
    match f.state {
        State::Live(g) if !g.held() => creature(f.look, f.x, f.y + bob, 1.0, f.facing, t),
        // つかまれている。小刻みに跳ねて、まわりに光を出す。
        State::Live(_) => {
            let wiggle = (t * 30.0).sin() * 7.0;
            let mut v = sparkles(f.x, f.y, t);
            v.extend(creature(f.look, f.x + wiggle, f.y, 1.05, f.facing, t * 4.0));
            v
        }
        // 釣れた。水しぶきを上げて、弧を描いてバケツへ飛ぶ。
        State::Caught { at } => {
            let p = (now.saturating_sub(at).as_secs_f32() / FLIGHT.as_secs_f32()).min(1.0);
            let x = f.x + (BUCKET.0 - f.x) * p;
            let y = f.y + (BUCKET.1 - f.y) * p - (p * std::f32::consts::PI).sin() * 220.0;
            let mut v = splash(f.x, WATER + 10.0, p, 1.0);
            v.extend(creature(f.look, x, y, 1.0 - 0.65 * p, f.facing, t * 4.0));
            v
        }
        // 逃げた。少し跳ねて、水に戻る。
        State::Slipped { at } => {
            let p = (now.saturating_sub(at).as_secs_f32() / HOP.as_secs_f32()).min(1.0);
            let hop = (p * std::f32::consts::PI).sin() * 70.0;
            let mut v = creature(f.look, f.x, f.y - hop, 1.0, -f.facing, t);
            v.extend(splash(f.x, f.y - 20.0, p, 0.5));
            v
        }
    }
}

/// 池にいるもの1匹。`(x, y)` が体の真ん中、`s` が大きさ、`t` は揺れの時刻。
///
/// **どれも体の幅 230・高さ 110（`BODY`）の枠に収まるくらいに描く。** つかめる
/// 範囲は種類によらず同じなので、見た目だけ大きいと、触ったのに逃げたように見える。
fn creature(c: Creature, x: f32, y: f32, s: f32, facing: f32, t: f32) -> Vec<Shape> {
    let (w, h) = (BODY.0 * s, BODY.1 * s);
    let pen = Pen { x, y, s, facing };
    match c.kind {
        Kind::Fish => fish(&pen, w, h, c.color),
        Kind::Puffer => puffer(&pen, h, c.color),
        Kind::Octopus => octopus(&pen, w, h, c.color, t),
        Kind::Shark => shark(&pen, w, h, c.color),
    }
}

/// 描く位置の基準。体の真ん中と向き。
struct Pen {
    x: f32,
    y: f32,
    s: f32,
    facing: f32,
}

impl Pen {
    /// 真ん中から `(dx, dy)` ずらした丸。`dx` は向いている側が正。
    fn dot(&self, dx: f32, dy: f32, r: f32, fill: Rgb) -> Shape {
        Shape::Circle {
            x: (self.x + dx * self.facing) as i32,
            y: (self.y + dy) as i32,
            r: (r as i32).max(1),
            fill,
        }
    }

    /// 真ん中から `(dx, dy)` ずらした、幅 `w`・高さ `h` の角丸（両端が丸い）。
    fn capsule(&self, dx: f32, dy: f32, w: f32, h: f32, fill: Rgb) -> Shape {
        let cx = self.x + dx * self.facing;
        Shape::RoundRect {
            rect: Rect {
                x: (cx - w / 2.0) as i32,
                y: (self.y + dy - h / 2.0) as i32,
                w: (w as i32).max(1),
                h: (h as i32).max(1),
            },
            r: (w.min(h) / 2.0) as i32,
            fill,
        }
    }

    /// 真ん中から `(dx, dy)` ずらした3点を結ぶ三角形。
    fn tri(&self, pts: [(f32, f32); 3], fill: Rgb) -> Shape {
        Shape::Triangle {
            points: pts.map(|(dx, dy)| ((self.x + dx * self.facing) as i32, (self.y + dy) as i32)),
            fill,
        }
    }

    /// 白目と黒目。黒目は向いている側へ少し寄せる。
    fn eye(&self, dx: f32, dy: f32, r: f32) -> [Shape; 2] {
        [
            self.dot(dx, dy, r.max(2.0), (250, 250, 250)),
            self.dot(dx + r * 0.25, dy, (r * 0.5).max(1.0), (20, 20, 20)),
        ]
    }
}

/// ふつうの魚。尾、背びれ、体、目の順に重ねる。
fn fish(p: &Pen, w: f32, h: f32, c: Rgb) -> Vec<Shape> {
    let dark = darken(c);
    let mut v = vec![
        p.tri(
            [
                (-w * 0.4, 0.0),
                (-w * 0.72, -h * 0.48),
                (-w * 0.72, h * 0.48),
            ],
            dark,
        ),
        p.tri(
            [
                (-w * 0.18, -h * 0.4),
                (w * 0.12, -h * 0.4),
                (-w * 0.24, -h * 0.78),
            ],
            dark,
        ),
        p.capsule(0.0, 0.0, w, h, c),
    ];
    v.extend(p.eye(w * 0.28, -h * 0.12, h * 0.14));
    v
}

/// フグ。まんまるの体に小さなトゲ、大きな目、おちょぼ口。
fn puffer(p: &Pen, h: f32, c: Rgb) -> Vec<Shape> {
    let dark = darken(c);
    let r = h * 0.62;
    let mut v = vec![p.tri(
        [
            (-r * 0.8, 0.0),
            (-r - h * 0.36, -h * 0.3),
            (-r - h * 0.36, h * 0.3),
        ],
        dark,
    )];
    v.extend((0..12).map(|i| {
        let a = i as f32 / 12.0 * std::f32::consts::TAU;
        p.dot(a.cos() * r, a.sin() * r, h * 0.08, dark)
    }));
    v.push(p.dot(0.0, 0.0, r, c));
    v.push(p.dot(-h * 0.05, h * 0.18, h * 0.12, dark));
    v.extend(p.eye(r * 0.4, -r * 0.3, h * 0.2));
    v.push(p.dot(r * 0.92, r * 0.18, h * 0.07, (120, 40, 40)));
    v
}

/// タコ。丸い頭に、ゆらゆらする足を4本。目は2つ。
fn octopus(p: &Pen, w: f32, h: f32, c: Rgb, t: f32) -> Vec<Shape> {
    let dark = darken(c);
    let mut v = Vec::new();
    for (leg, &fx) in [-0.36f32, -0.12, 0.12, 0.36].iter().enumerate() {
        for k in 0..3 {
            let sway = (t * 3.0 + leg as f32 * 1.3 + k as f32 * 0.8).sin() * w * 0.04;
            v.push(p.dot(
                fx * w * 0.5 + sway,
                h * (0.22 + 0.2 * k as f32),
                h * (0.16 - 0.03 * k as f32),
                if k % 2 == 0 { c } else { dark },
            ));
        }
    }
    v.push(p.dot(0.0, -h * 0.15, h * 0.55, c));
    v.extend(p.eye(-h * 0.2, -h * 0.2, h * 0.13));
    v.extend(p.eye(h * 0.2, -h * 0.2, h * 0.13));
    v.push(p.dot(0.0, h * 0.08, h * 0.08, dark));
    v
}

/// サメ。長い体に尖った鼻、白いお腹、三角の高い背びれ、二股の尾、えら。
fn shark(p: &Pen, w: f32, h: f32, c: Rgb) -> Vec<Shape> {
    let dark = darken(c);
    let belly = (232, 238, 244);
    let mut v = vec![
        // 二股の尾。上が大きい。
        p.tri(
            [
                (-w * 0.48, -h * 0.08),
                (-w * 0.78, -h * 0.62),
                (-w * 0.64, 0.0),
            ],
            dark,
        ),
        p.tri(
            [
                (-w * 0.48, h * 0.06),
                (-w * 0.74, h * 0.46),
                (-w * 0.64, 0.0),
            ],
            dark,
        ),
        // 背びれ。高く、後ろへ倒す。
        p.tri(
            [
                (-w * 0.14, -h * 0.3),
                (w * 0.14, -h * 0.3),
                (-w * 0.16, -h * 0.88),
            ],
            dark,
        ),
        // 尖った鼻。体より先に描いて、体の丸い先から突き出させる。
        p.tri(
            [
                (w * 0.46, -h * 0.34),
                (w * 0.46, h * 0.3),
                (w * 0.7, h * 0.02),
            ],
            c,
        ),
        p.capsule(0.0, 0.0, w * 1.1, h * 0.8, c),
        p.capsule(w * 0.04, h * 0.17, w * 0.86, h * 0.36, belly),
        // 胸びれ。
        p.tri(
            [
                (w * 0.02, h * 0.2),
                (w * 0.22, h * 0.2),
                (-w * 0.04, h * 0.62),
            ],
            dark,
        ),
    ];
    for gx in [0.24, 0.3] {
        v.push(p.capsule(w * gx, -h * 0.04, (4.0 * p.s).max(1.0), h * 0.28, dark));
    }
    v.extend(p.eye(w * 0.42, -h * 0.13, h * 0.1));
    v
}

/// つかんでいる魚のまわりの光。
fn sparkles(x: f32, y: f32, t: f32) -> Vec<Shape> {
    (0..5)
        .map(|i| {
            let a = i as f32 / 5.0 * std::f32::consts::TAU + t * 3.0;
            Shape::Circle {
                x: (x + a.cos() * BODY.0 * 0.7) as i32,
                y: (y + a.sin() * BODY.1 * 0.9) as i32,
                r: 7,
                fill: (255, 240, 150),
            }
        })
        .collect()
}

/// 水しぶき。粒が上へ散って落ちる。`p` は 0〜1 の進み、`size` は勢い。
fn splash(x: f32, y: f32, p: f32, size: f32) -> Vec<Shape> {
    if p >= 1.0 {
        return Vec::new();
    }
    (0..9)
        .map(|i| {
            let a = -std::f32::consts::PI * (0.15 + 0.7 * i as f32 / 8.0);
            let v = 260.0 * size;
            let (dx, dy) = (a.cos() * v * p, a.sin() * v * p + 400.0 * size * p * p);
            Shape::Circle {
                x: (x + dx) as i32,
                y: (y + dy) as i32,
                r: ((12.0 * size * (1.0 - p)) as i32).max(2),
                fill: (190, 230, 255),
            }
        })
        .collect()
}

/// 右上のバケツと、釣った魚。多ければ最後の数匹だけ見せる。
fn bucket(caught: &[Creature], now: Duration) -> Vec<Shape> {
    let (bx, by) = (BUCKET.0 as i32, BUCKET.1 as i32);
    let mut v = vec![
        Shape::RoundRect {
            rect: Rect {
                x: bx - 80,
                y: by - 50,
                w: 160,
                h: 110,
            },
            r: 22,
            fill: (150, 105, 60),
        },
        Shape::RoundRect {
            rect: Rect {
                x: bx - 90,
                y: by - 60,
                w: 180,
                h: 20,
            },
            r: 10,
            fill: (185, 140, 90),
        },
    ];
    // 頭がバケツから覗く。
    for (i, &c) in caught.iter().rev().take(5).enumerate() {
        let x = bx - 50 + (i as i32 % 5) * 25;
        v.extend(creature(
            c,
            x as f32,
            (by - 62) as f32,
            0.22,
            1.0,
            now.as_secs_f32(),
        ));
    }
    v
}

/// `area` に魚を並べる。数が多ければ小さくする。ゆっくり揺らす。
fn lineup(area: Rect, shown: &[Creature], now: Duration) -> Vec<Shape> {
    let n = shown.len().max(1);
    let cols = (1..=n).find(|c| c * c * 9 >= n * 16).unwrap_or(n).max(1);
    let rows = n.div_ceil(cols);
    let (cw, ch) = (area.w / cols as i32, area.h / rows as i32);
    // 一番大きいサメ（尾まで体の幅の 1.5 倍、背びれまで高さの 1.75 倍）が
    // 升の 8 割に収まる大きさ。
    let s = (cw as f32 * 0.8 / BODY.0 / 1.5).min(ch as f32 * 0.8 / BODY.1 / 1.75);
    let t = now.as_secs_f32();
    shown
        .iter()
        .enumerate()
        .flat_map(|(i, &c)| {
            let bob = (t * 2.0 + i as f32 * 0.7).sin() * BODY.1 * s * 0.08;
            let facing = if (i / cols) % 2 == 0 { 1.0 } else { -1.0 };
            creature(
                c,
                (area.x + (i % cols) as i32 * cw + cw / 2) as f32,
                (area.y + (i / cols) as i32 * ch + ch / 2) as f32 + bob,
                s,
                facing,
                t,
            )
        })
        .collect()
}

/// 調整用。動いた向き（上は緑、下は赤）と、魚ごとの振り上げの強さ。
fn debug(flow: &Flowed, pond: &Pond) -> Vec<Shape> {
    let mut v: Vec<Shape> = flow
        .vectors()
        .map(|(x, y, _, vy)| Shape::Circle {
            x,
            y,
            r: 3,
            fill: if vy < 0.0 {
                (60, 255, 90)
            } else {
                (255, 70, 70)
            },
        })
        .collect();
    let need = pond.need().max(1.0);
    for f in pond.fish() {
        let State::Live(grip) = f.state else {
            continue;
        };
        let Some(w) = grip.swing() else {
            continue;
        };
        // 魚の左右に、振り上げの強さの棒。上の線まで伸びれば足りている。
        for (dx, speed) in [(-BODY.0 * 0.8, w.left), (BODY.0 * 0.8, w.right)] {
            let full = 200.0;
            let h = (speed / need * full).min(full * 1.5) as i32;
            let x = (f.x + dx) as i32;
            let base = (f.y + BODY.1) as i32;
            v.push(Shape::RoundRect {
                rect: Rect {
                    x: x - 10,
                    y: base - h,
                    w: 20,
                    h: h.max(1),
                },
                r: 4,
                fill: if speed >= need {
                    (60, 255, 90)
                } else {
                    (255, 200, 60)
                },
            });
            v.push(Shape::RoundRect {
                rect: Rect {
                    x: x - 18,
                    y: base - full as i32 - 2,
                    w: 36,
                    h: 4,
                },
                r: 2,
                fill: (250, 250, 250),
            });
        }
    }
    v
}

fn darken(c: Rgb) -> Rgb {
    let m = |v: u8| (v as f32 * 0.7) as u8;
    (m(c.0), m(c.1), m(c.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 図形の外接する長方形。`(左, 上, 右, 下)`。
    fn bounds(s: &Shape) -> (i32, i32, i32, i32) {
        match *s {
            Shape::Circle { x, y, r, .. } => (x - r, y - r, x + r, y + r),
            Shape::RoundRect { rect, .. } => (rect.x, rect.y, rect.x + rect.w, rect.y + rect.h),
            Shape::Triangle { points, .. } => {
                let xs = points.map(|p| p.0);
                let ys = points.map(|p| p.1);
                (
                    *xs.iter().min().unwrap(),
                    *ys.iter().min().unwrap(),
                    *xs.iter().max().unwrap(),
                    *ys.iter().max().unwrap(),
                )
            }
            _ => (0, 0, 0, 0),
        }
    }

    /// **札に縮めても、はみ出さない。**
    #[test]
    fn the_icon_stays_inside_its_area() {
        let area = Rect {
            x: 100,
            y: 50,
            w: 400,
            h: 300,
        };
        for s in icon(area) {
            if let Shape::Circle { x, y, r, .. } = s {
                assert!(x - r >= area.x && x + r <= area.x + area.w, "{s:?}");
                assert!(y - r >= area.y && y + r <= area.y + area.h, "{s:?}");
            }
        }
    }

    /// 釣った数がどれだけ多くても、並べた魚は画面に収まり、点数に重ならない。
    #[test]
    fn the_lineup_fits_below_the_score() {
        for n in [0, 1, 7, 30, 120] {
            let caught: Vec<Creature> = (0..n)
                .map(|i| Creature {
                    kind: Kind::ALL[i % Kind::ALL.len()],
                    color: (255, 0, 0),
                })
                .collect();
            let scene = finale(&caught, Duration::from_secs(1));
            let Some(&Shape::Text { y, px, text, .. }) = scene.shapes.first() else {
                panic!("点数が出ていない");
            };
            assert_eq!(text, score(n));
            let bottom = y + px / 2;
            for s in &scene.shapes[1..] {
                if matches!(s, Shape::Text { .. }) {
                    continue;
                }
                let (left, top, right, low) = bounds(s);
                assert!(top >= bottom, "{n}: 点数に重なった: {s:?}");
                assert!(
                    low <= HEIGHT && left >= 0 && right <= WIDTH,
                    "{n}: はみ出した: {s:?}"
                );
            }
        }
    }

    /// **どの種類も、つかめる範囲（`BODY` の枠の 1.6 倍）に収まる。** 見た目だけ
    /// 大きいと、触ったのに逃げたように見える。
    #[test]
    fn every_kind_fits_its_catch_box() {
        for kind in Kind::ALL {
            for facing in [1.0, -1.0] {
                let c = Creature {
                    kind,
                    color: (200, 100, 50),
                };
                for s in creature(c, 0.0, 0.0, 1.0, facing, 1.3) {
                    let (l, t, r, b) = bounds(&s);
                    let (hw, hh) = ((BODY.0 * 0.8) as i32, (BODY.1 * 0.9) as i32);
                    assert!(
                        l >= -hw && r <= hw && t >= -hh && b <= hh,
                        "{kind:?}: {s:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_score_uses_only_the_listed_glyphs() {
        for n in [0, 3, 42, 999, 5000] {
            for ch in score(n).chars() {
                assert!(GLYPHS.contains(ch), "{n}: 「{ch}」が GLYPHS に無い");
            }
        }
        assert_eq!(score(5000), score(999));
    }

    /// 池は下半分だけ青い。上は暗くした鏡のまま。
    #[test]
    fn only_the_lower_half_is_a_pond() {
        let img = Image::filled(320, 180, (100, 100, 100));
        let p = pond_over(&img);
        let (top, low) = (p.at(10, 10), p.at(10, 170));
        assert_eq!(top, (60, 60, 60));
        assert!(low.2 > low.0 + 40, "下が青くない: {low:?}");
    }
}
