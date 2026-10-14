//! 何を映すか。鏡に映った自分、池、魚。
//!
//! 画素は知らない。図形を並べて `kit::Scene` にするところまでで、塗るのは
//! `home` の描き手。池は鏡の絵の下半分を青く染めて作る。

use std::sync::OnceLock;
use std::time::Duration;

use kit::{Image, Rect, Rgb, Scene, Shape, HEIGHT, WIDTH};
use vision::Flowed;

use crate::pond::{Fish, Pond, State, BODY, COLORS, FLIGHT, HOP, WATER};

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
    shapes.extend(bucket(pond.caught()));
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
pub fn finale(caught: &[Rgb], now: Duration) -> Scene {
    let colors: Vec<Rgb> = if caught.is_empty() {
        COLORS.to_vec()
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
        &colors,
        now,
    ));
    Scene {
        shapes,
        caption: format!("── {} ──", score(caught.len())),
        ..Scene::default()
    }
}

/// メニューの札に描く絵。水と、魚を2匹。
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
    shapes.extend(fish(
        area.x as f32 + area.w as f32 * 0.3,
        area.y as f32 + area.h as f32 * 0.42,
        s,
        1.0,
        COLORS[0],
    ));
    shapes.extend(fish(
        area.x as f32 + area.w as f32 * 0.68,
        area.y as f32 + area.h as f32 * 0.72,
        s,
        -1.0,
        COLORS[3],
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
        State::Swim { .. } => fish(f.x, f.y + bob, 1.0, f.facing, f.color),
        // つかまれている。小刻みに跳ねて、まわりに光を出す。
        State::Held { .. } => {
            let wiggle = (t * 30.0).sin() * 7.0;
            let mut v = sparkles(f.x, f.y, t);
            v.extend(fish(f.x + wiggle, f.y, 1.05, f.facing, f.color));
            v
        }
        // 釣れた。水しぶきを上げて、弧を描いてバケツへ飛ぶ。
        State::Caught { at } => {
            let p = (now.saturating_sub(at).as_secs_f32() / FLIGHT.as_secs_f32()).min(1.0);
            let x = f.x + (BUCKET.0 - f.x) * p;
            let y = f.y + (BUCKET.1 - f.y) * p - (p * std::f32::consts::PI).sin() * 220.0;
            let mut v = splash(f.x, WATER + 10.0, p, 1.0);
            v.extend(fish(x, y, 1.0 - 0.65 * p, f.facing, f.color));
            v
        }
        // 逃げた。少し跳ねて、水に戻る。
        State::Slipped { at } => {
            let p = (now.saturating_sub(at).as_secs_f32() / HOP.as_secs_f32()).min(1.0);
            let hop = (p * std::f32::consts::PI).sin() * 70.0;
            let mut v = fish(f.x, f.y - hop, 1.0, -f.facing, f.color);
            v.extend(splash(f.x, f.y - 20.0, p, 0.5));
            v
        }
    }
}

/// 魚1匹。尾、背びれ、体、目の順に重ねる。`(x, y)` が体の真ん中、`s` が大きさ。
fn fish(x: f32, y: f32, s: f32, facing: f32, c: Rgb) -> Vec<Shape> {
    let (w, h) = (BODY.0 * s, BODY.1 * s);
    let dark = darken(c);
    let at = |dx: f32, dy: f32| ((x + dx * facing) as i32, (y + dy) as i32);
    let (tx, ty) = at(-w * 0.55, 0.0);
    let (fx, fy) = at(-w * 0.05, -h * 0.45);
    let (ex, ey) = at(w * 0.28, -h * 0.12);
    vec![
        Shape::Circle {
            x: tx,
            y: ty,
            r: (h * 0.42) as i32,
            fill: dark,
        },
        Shape::Circle {
            x: fx,
            y: fy,
            r: (h * 0.25) as i32,
            fill: dark,
        },
        Shape::RoundRect {
            rect: Rect {
                x: (x - w / 2.0) as i32,
                y: (y - h / 2.0) as i32,
                w: w as i32,
                h: h as i32,
            },
            r: (h / 2.0) as i32,
            fill: c,
        },
        Shape::Circle {
            x: ex,
            y: ey,
            r: (h * 0.14).max(2.0) as i32,
            fill: (250, 250, 250),
        },
        Shape::Circle {
            x: ex + (h * 0.03 * facing) as i32,
            y: ey,
            r: (h * 0.07).max(1.0) as i32,
            fill: (20, 20, 20),
        },
    ]
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
fn bucket(caught: &[Rgb]) -> Vec<Shape> {
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
        v.extend(fish(x as f32, (by - 62) as f32, 0.22, 1.0, c));
    }
    v
}

/// `area` に魚を並べる。数が多ければ小さくする。ゆっくり揺らす。
fn lineup(area: Rect, colors: &[Rgb], now: Duration) -> Vec<Shape> {
    let n = colors.len().max(1);
    let cols = (1..=n).find(|c| c * c * 9 >= n * 16).unwrap_or(n).max(1);
    let rows = n.div_ceil(cols);
    let (cw, ch) = (area.w / cols as i32, area.h / rows as i32);
    // 体の幅が升の 7 割、高さが升の 6 割に収まる大きさ。
    let s = (cw as f32 * 0.7 / BODY.0 / 1.2).min(ch as f32 * 0.6 / BODY.1);
    colors
        .iter()
        .enumerate()
        .flat_map(|(i, &c)| {
            let bob = (now.as_secs_f32() * 2.0 + i as f32 * 0.7).sin() * BODY.1 * s * 0.1;
            let facing = if (i / cols) % 2 == 0 { 1.0 } else { -1.0 };
            fish(
                (area.x + (i % cols) as i32 * cw + cw / 2) as f32,
                (area.y + (i / cols) as i32 * ch + ch / 2) as f32 + bob,
                s,
                facing,
                c,
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
        let State::Held { swing: Some(w), .. } = f.state else {
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
            let scene = finale(&vec![(255, 0, 0); n], Duration::from_secs(1));
            let Some(&Shape::Text { y, px, text, .. }) = scene.shapes.first() else {
                panic!("点数が出ていない");
            };
            assert_eq!(text, score(n));
            let bottom = y + px / 2;
            for s in &scene.shapes[1..] {
                let (top, low, left, right) = match *s {
                    Shape::Circle { x, y, r, .. } => (y - r, y + r, x - r, x + r),
                    Shape::RoundRect { rect, .. } => {
                        (rect.y, rect.y + rect.h, rect.x, rect.x + rect.w)
                    }
                    _ => continue,
                };
                assert!(top >= bottom, "{n}: 点数に重なった: {s:?}");
                assert!(
                    low <= HEIGHT && left >= 0 && right <= WIDTH,
                    "{n}: はみ出した: {s:?}"
                );
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
