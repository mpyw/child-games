//! 何を映すか。鏡に映った自分、たこ焼き器、たこ焼き、舟皿。
//!
//! 画素は知らない。図形を並べて `kit::Scene` にするところまでで、塗るのは
//! `home` の描き手。鉄板は鏡の絵の下の部分を染めて作る。

use std::sync::OnceLock;
use std::time::Duration;

use kit::{Image, Rect, Rgb, Scene, Shape, HEIGHT, WIDTH};
use vision::Flowed;

use crate::plate::{Hole, Plate, Stage, BALL, DROP, FLIGHT, FLIP, HOLE, HOLES, JIGGLE, POUR};

/// 鏡の暗さ。**たこ焼きが主役。** 明るいままだとたこ焼きが紛れる。
const MIRROR: f32 = 0.6;
/// 鉄板の上端と色、混ぜる割合。透けて自分の手が見えるくらいに留める。
const IRON_TOP: f32 = 285.0;
const IRON: Rgb = (70, 55, 48);
const TINT: f32 = 0.55;
/// 穴の色。
const PIT: Rgb = (38, 30, 26);
/// 舟皿の位置。右上。
const BOAT: (f32, f32) = (WIDTH as f32 - 150.0, 105.0);

/// 生地の色。焼けていない。
const RAW: Rgb = (248, 236, 196);
/// 片面が焼けた色。
const GOLD: Rgb = (232, 180, 96);
/// 裏返したあとの、焼けた面の色。
const BROWN: Rgb = (205, 128, 52);
/// 両面が焼けた色。
const DONE: Rgb = (186, 104, 40);
/// ソース。
const SAUCE: Rgb = (92, 46, 26);

/// 遊んでいる間の場面。`motion` が `Some` なら、動きの向きと強さも出す。
pub fn playing(
    mirror: &Image,
    plate: &Plate,
    flowed: &Flowed,
    now: Duration,
    motion: bool,
) -> Scene {
    let t = now.as_secs_f32();
    let mut shapes = Vec::new();
    for &(x, y) in &HOLES {
        shapes.push(Shape::Circle {
            x: x as i32,
            y: y as i32,
            r: HOLE as i32,
            fill: PIT,
        });
    }
    shapes.extend(boat(plate.served().min(6), t));
    for h in plate.holes() {
        shapes.extend(ball_in_hole(h, now));
    }
    // **動いた所に小さな光を出す。** 自分の指がどこにあるか見える。
    for (x, y) in flowed.spots() {
        shapes.push(Shape::Circle {
            x,
            y,
            r: 3,
            fill: (255, 255, 220),
        });
    }
    if motion {
        shapes.extend(debug(flowed, plate));
    }
    Scene {
        image: Some(iron_over(mirror)),
        shapes,
        caption: format!(
            "── やけた {}こ / いる はやさ {:.0} ──",
            plate.served(),
            plate.need()
        ),
    }
}

/// 点数に使う字。`Game::glyphs` で `home` に渡し、フォントに入っているかを見させる。
pub const GLYPHS: &str = "0123456789こ やけたね！";

/// 点数の上限。1分で焼けるのはせいぜい数十なので、ここで頭打ちにする。
const MAX_SCORE: usize = 999;

/// 焼けた数を書いた字。「12こ やけたね！」。
///
/// **一度だけ作って持っておく。** `Shape::Text` は `&'static str` しか
/// 持てない。毎フレーム作って手放せない文字列にすると、遊ぶほど漏れる。
pub fn score(n: usize) -> &'static str {
    static TEXTS: OnceLock<Vec<String>> = OnceLock::new();
    let texts = TEXTS.get_or_init(|| {
        (0..=MAX_SCORE)
            .map(|n| format!("{n}こ やけたね！"))
            .collect()
    });
    &texts[n.min(MAX_SCORE)]
}

/// 曲が終わったあと。**上に焼けた数を出し、下にソースのかかったたこ焼きを
/// 並べて祝う。**
///
/// 1つも焼けなかったときも、6つ並べて同じように祝う。2歳児に「できなかった」を
/// 見せる理由は無い。
pub fn finale(served: usize, now: Duration) -> Scene {
    let top = HEIGHT * 28 / 100;
    let mut shapes = vec![Shape::Text {
        x: WIDTH / 2,
        y: HEIGHT * 14 / 100,
        px: HEIGHT / 8,
        text: score(served),
        fill: (250, 250, 250),
    }];
    shapes.extend(lineup(
        Rect {
            x: 0,
            y: top,
            w: WIDTH,
            h: HEIGHT - top,
        },
        if served == 0 { 6 } else { served },
        now,
    ));
    Scene {
        shapes,
        caption: format!("── {} ──", score(served)),
        ..Scene::default()
    }
}

/// メニューの札に描く絵。鉄板に、たこ焼きを3つ。
pub fn icon(area: Rect) -> Vec<Shape> {
    let mut v = vec![Shape::RoundRect {
        rect: area.inset(area.h / 12),
        r: area.h / 8,
        fill: IRON,
    }];
    let r = area.h as f32 * 0.16;
    for (i, &(fx, fy)) in [(0.27, 0.55), (0.5, 0.55), (0.73, 0.55)].iter().enumerate() {
        let (x, y) = (
            area.x as f32 + area.w as f32 * fx,
            area.y as f32 + area.h as f32 * fy,
        );
        v.push(Shape::Circle {
            x: x as i32,
            y: y as i32,
            r: (r * 1.2) as i32,
            fill: PIT,
        });
        v.extend(match i {
            0 => raw_top(x, y, r, 0.6, 0.0),
            1 => ball(x, y, r, BROWN),
            _ => topped(x, y, r),
        });
    }
    v
}

/// 鏡の絵の下の部分を染めて鉄板にする。上は少し暗くする。
fn iron_over(mirror: &Image) -> Image {
    let line = (mirror.height as f32 * IRON_TOP / HEIGHT as f32) as usize;
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
                (m(r, IRON.0), m(g, IRON.1), m(b, IRON.2))
            }
        })
        .collect();
    Image {
        width: mirror.width,
        height: mirror.height,
        pixels,
    }
}

/// 穴の中のたこ焼き1つ。様子で描き方を変える。
fn ball_in_hole(h: &Hole, now: Duration) -> Vec<Shape> {
    let t = now.as_secs_f32();
    let since = |at: Duration| now.saturating_sub(at).as_secs_f32();
    // まだ焼けていないのをつついたら、少しのあいだ揺れる。
    let jiggle = match h.jiggled {
        Some(at) if now.saturating_sub(at) < JIGGLE => (since(at) * 60.0).sin() * 6.0,
        _ => 0.0,
    };
    let (x, y) = (h.x + jiggle, h.y);
    match h.stage {
        Stage::Empty { .. } => Vec::new(),
        // 生地を流している。丸が広がっていく。
        Stage::Pour { at } => {
            let p = (since(at) / POUR.as_secs_f32()).min(1.0);
            raw_top(x, y, BALL * (0.3 + 0.7 * p), 0.0, t)
        }
        // 片面が焼けていく。縁から色づき、湯気が少しずつ増える。
        Stage::CookA { .. } => {
            let d = h.doneness(now).unwrap_or(0.0);
            let mut v = raw_top(x, y, BALL, d, t);
            v.extend(steam(x, y, d * 0.6, t));
            v
        }
        // 片面が焼けた。縁が焼け色になって、ぴょこぴょこ跳ね、湯気が立つ。
        Stage::ReadyA { at } => {
            let hop = (since(at) * 6.0).sin().abs() * 8.0;
            let mut v = raw_top(x, y - hop, BALL, 1.0, t);
            v.extend(steam(x, y, 1.0, t));
            v.extend(twinkle(x, y, t, (255, 230, 120)));
            v
        }
        // 裏返っている。縦につぶれて、焼けた面が上に来る。
        Stage::Flip { at } => {
            let p = (since(at) / FLIP.as_secs_f32()).min(1.0);
            let squash = (p * std::f32::consts::PI).cos().abs().max(0.15);
            let c = if p < 0.5 { GOLD } else { BROWN };
            let lift = (p * std::f32::consts::PI).sin() * 30.0;
            vec![Shape::RoundRect {
                rect: Rect {
                    x: (x - BALL) as i32,
                    y: (y - lift - BALL * squash) as i32,
                    w: (BALL * 2.0) as i32,
                    h: ((BALL * 2.0 * squash) as i32).max(2),
                },
                r: (BALL * squash) as i32,
                fill: c,
            }]
        }
        // 反対側が焼けていく。焼けた丸い面が見えている。
        Stage::CookB { .. } => {
            let d = h.doneness(now).unwrap_or(0.0);
            let mut v = ball(x, y, BALL, mix(BROWN, DONE, d));
            v.extend(steam(x, y, d * 0.6, t));
            v
        }
        // 両面が焼けた。キラキラして跳ねる。つかまれていれば小刻みに震える。
        Stage::ReadyB { at, grip, .. } => {
            let (dx, dy) = if grip.held() {
                ((t * 30.0).sin() * 6.0, -6.0)
            } else {
                (0.0, -(since(at) * 6.0).sin().abs() * 8.0)
            };
            let mut v = ball(x + dx, y + dy, BALL, DONE);
            v.extend(twinkle(x, y, t, (255, 245, 170)));
            v
        }
        // 舟皿へ飛ぶ。弧を描いて小さくなる。
        Stage::Flying { at } => {
            let p = (since(at) / FLIGHT.as_secs_f32()).min(1.0);
            let fx = h.x + (BOAT.0 - h.x) * p;
            let fy = h.y + (BOAT.1 - h.y) * p - (p * std::f32::consts::PI).sin() * 200.0;
            ball(fx, fy, BALL * (1.0 - 0.6 * p), DONE)
        }
        // 落ちた。少し跳ねて戻る。
        Stage::Dropped { at } => {
            let p = (since(at) / DROP.as_secs_f32()).min(1.0);
            let hop = (p * std::f32::consts::PI).sin() * 40.0;
            ball(x, y - hop, BALL, DONE)
        }
    }
}

/// 生地の面が上のたこ焼き。`d` は焼け具合で、縁から色づく。具が覗いている。
fn raw_top(x: f32, y: f32, r: f32, d: f32, t: f32) -> Vec<Shape> {
    let rim = mix(RAW, GOLD, d);
    let mut v = vec![
        Shape::Circle {
            x: x as i32,
            y: y as i32,
            r: r as i32,
            fill: rim,
        },
        Shape::Circle {
            x: x as i32,
            y: y as i32,
            r: (r * (0.85 - 0.25 * d)) as i32,
            fill: mix(RAW, GOLD, d * 0.4),
        },
    ];
    // たこ（赤）と、ねぎ（緑）と、紅しょうが。
    for (i, (dx, dy, c)) in [
        (-0.3, -0.2, (210, 70, 70)),
        (0.25, 0.15, (210, 70, 70)),
        (0.1, -0.35, (90, 170, 80)),
        (-0.15, 0.3, (90, 170, 80)),
        (0.35, -0.1, (230, 90, 110)),
    ]
    .iter()
    .enumerate()
    {
        v.push(Shape::Circle {
            x: (x + dx * r) as i32,
            y: (y + dy * r) as i32,
            r: (r * 0.1).max(2.0) as i32,
            fill: *c,
        });
        // 焼けていく生地の泡。ぷつぷつと出たり消えたりする。
        if d > 0.2 && ((t * 2.0 + i as f32 * 1.7).sin() > 0.6) {
            v.push(Shape::Circle {
                x: (x - dy * r * 0.9) as i32,
                y: (y + dx * r * 0.9) as i32,
                r: (r * 0.07).max(2.0) as i32,
                fill: (255, 248, 225),
            });
        }
    }
    v
}

/// 焼けた丸い面が上のたこ焼き。光を1つ入れて丸く見せる。
fn ball(x: f32, y: f32, r: f32, c: Rgb) -> Vec<Shape> {
    vec![
        Shape::Circle {
            x: x as i32,
            y: y as i32,
            r: (r as i32).max(2),
            fill: c,
        },
        Shape::Circle {
            x: (x - r * 0.35) as i32,
            y: (y - r * 0.35) as i32,
            r: (r * 0.22).max(1.0) as i32,
            fill: mix(c, (255, 240, 210), 0.5),
        },
    ]
}

/// ソース・マヨネーズ・青のり・かつお節がかかったたこ焼き。お祝いと舟皿に使う。
///
/// **ソースは角丸で描く。** 丸（`Shape::Circle`）には描き手が縁を付けるので、
/// 暗いソースに明るい輪が出て、別の食べ物に見える。角丸を正方形にして角を
/// 半分丸めれば、縁の無い丸になる。
fn topped(x: f32, y: f32, r: f32) -> Vec<Shape> {
    let mut v = ball(x, y, r, DONE);
    let sr = r * 0.8;
    v.push(Shape::RoundRect {
        rect: Rect {
            x: (x - sr) as i32,
            y: (y - r * 0.18 - sr) as i32,
            w: (sr * 2.0) as i32,
            h: (sr * 2.0) as i32,
        },
        r: sr as i32,
        fill: SAUCE,
    });
    // マヨネーズ。波打つ線を2本、点を並べて描く。
    let mayo = (250, 246, 228);
    for (row, dy) in [-0.35f32, 0.05].iter().enumerate() {
        for k in 0..15 {
            let u = k as f32 / 14.0;
            let px = x + (u - 0.5) * r * 1.2;
            let py = y + dy * r + (u * 9.0 + row as f32).sin() * r * 0.1;
            v.push(Shape::RoundRect {
                rect: Rect {
                    x: (px - r * 0.07) as i32,
                    y: (py - r * 0.07) as i32,
                    w: (r * 0.14).max(2.0) as i32,
                    h: (r * 0.14).max(2.0) as i32,
                },
                r: (r * 0.07).max(1.0) as i32,
                fill: mayo,
            });
        }
    }
    // かつお節と青のり。
    for (dx, dy) in [(-0.25, -0.6), (0.3, -0.45)] {
        v.push(Shape::RoundRect {
            rect: Rect {
                x: (x + dx * r) as i32,
                y: (y + dy * r) as i32,
                w: (r * 0.28) as i32,
                h: (r * 0.12).max(2.0) as i32,
            },
            r: (r * 0.05).max(1.0) as i32,
            fill: (214, 160, 120),
        });
    }
    for (dx, dy) in [
        (-0.4, -0.15),
        (0.1, -0.55),
        (0.45, -0.1),
        (-0.05, 0.3),
        (0.3, 0.25),
    ] {
        v.push(Shape::RoundRect {
            rect: Rect {
                x: (x + dx * r) as i32,
                y: (y + dy * r) as i32,
                w: (r * 0.09).max(2.0) as i32,
                h: (r * 0.09).max(2.0) as i32,
            },
            r: 1,
            fill: (70, 140, 50),
        });
    }
    v
}

/// 湯気。`k` は量（0〜1）。白い粒が上へ昇って消える。
fn steam(x: f32, y: f32, k: f32, t: f32) -> Vec<Shape> {
    let n = (k * 4.0).round() as usize;
    (0..n)
        .map(|i| {
            let p = (t * 0.8 + i as f32 / 4.0).fract();
            Shape::Circle {
                x: (x + ((t * 2.0 + i as f32 * 2.1).sin() * 18.0) + (i as f32 - 1.5) * 22.0) as i32,
                y: (y - BALL * 0.7 - p * 110.0) as i32,
                r: ((16.0 * (1.0 - p)) as i32).max(3),
                fill: mix((248, 248, 248), IRON, p * 0.45),
            }
        })
        .collect()
}

/// 焼けた合図のキラキラ。たこ焼きのまわりを回る。
fn twinkle(x: f32, y: f32, t: f32, c: Rgb) -> Vec<Shape> {
    (0..4)
        .map(|i| {
            let a = i as f32 / 4.0 * std::f32::consts::TAU + t * 2.5;
            Shape::Circle {
                x: (x + a.cos() * BALL * 1.1) as i32,
                y: (y + a.sin() * BALL * 1.1) as i32,
                r: 6,
                fill: c,
            }
        })
        .collect()
}

/// 右上の舟皿と、乗ったたこ焼き（最大6つ）。
fn boat(n: usize, _t: f32) -> Vec<Shape> {
    let (bx, by) = BOAT;
    let mut v = vec![
        Shape::RoundRect {
            rect: Rect {
                x: (bx - 130.0) as i32,
                y: (by - 30.0) as i32,
                w: 260,
                h: 80,
            },
            r: 30,
            fill: (205, 170, 120),
        },
        Shape::RoundRect {
            rect: Rect {
                x: (bx - 118.0) as i32,
                y: (by - 22.0) as i32,
                w: 236,
                h: 62,
            },
            r: 24,
            fill: (230, 200, 150),
        },
    ];
    for i in 0..n {
        let (cx, cy) = (
            bx - 85.0 + (i % 3) as f32 * 85.0,
            by + 5.0 - (i / 3) as f32 * 30.0,
        );
        v.extend(topped(cx, cy, 28.0));
    }
    v
}

/// `area` にソースのかかったたこ焼きを並べる。数が多ければ小さくする。
fn lineup(area: Rect, n: usize, now: Duration) -> Vec<Shape> {
    let n = n.max(1);
    let cols = (1..=n).find(|c| c * c * 9 >= n * 16).unwrap_or(n).max(1);
    let rows = n.div_ceil(cols);
    let (cw, ch) = (area.w / cols as i32, area.h / rows as i32);
    let r = (cw.min(ch) as f32 * 0.38).min(BALL * 1.3);
    let t = now.as_secs_f32();
    (0..n)
        .flat_map(|i| {
            let bob = (t * 2.0 + i as f32 * 0.7).sin() * r * 0.06;
            topped(
                (area.x + (i % cols) as i32 * cw + cw / 2) as f32,
                (area.y + (i / cols) as i32 * ch + ch / 2) as f32 + bob,
                r,
            )
        })
        .collect()
}

/// 調整用。動いた向き（上は緑、下は赤）と、つかんでいる穴の振り上げの強さ。
fn debug(flow: &Flowed, plate: &Plate) -> Vec<Shape> {
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
    let need = plate.need().max(1.0);
    for h in plate.holes() {
        let Stage::ReadyB { grip, .. } = h.stage else {
            continue;
        };
        let Some(w) = grip.swing() else {
            continue;
        };
        for (dx, speed) in [(-HOLE * 1.2, w.left), (HOLE * 1.2, w.right)] {
            let full = 180.0;
            let hgt = (speed / need * full).min(full * 1.5) as i32;
            let x = (h.x + dx) as i32;
            let base = (h.y + HOLE) as i32;
            v.push(Shape::RoundRect {
                rect: Rect {
                    x: x - 10,
                    y: base - hgt,
                    w: 20,
                    h: hgt.max(1),
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

/// `a` から `b` へ、`k`（0〜1）だけ寄せた色。
fn mix(a: Rgb, b: Rgb, k: f32) -> Rgb {
    let k = k.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k) as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(s: &Shape) -> Option<(i32, i32, i32, i32)> {
        match *s {
            Shape::Circle { x, y, r, .. } => Some((x - r, y - r, x + r, y + r)),
            Shape::RoundRect { rect, .. } => {
                Some((rect.x, rect.y, rect.x + rect.w, rect.y + rect.h))
            }
            _ => None,
        }
    }

    #[test]
    fn the_icon_stays_inside_its_area() {
        let area = Rect {
            x: 100,
            y: 50,
            w: 400,
            h: 300,
        };
        for s in icon(area) {
            let (l, t, r, b) = bounds(&s).unwrap();
            assert!(l >= area.x && r <= area.x + area.w, "{s:?}");
            assert!(t >= area.y && b <= area.y + area.h, "{s:?}");
        }
    }

    /// 焼けた数がどれだけ多くても、並べたたこ焼きは画面に収まり、点数に重ならない。
    #[test]
    fn the_lineup_fits_below_the_score() {
        for n in [0, 1, 7, 30, 120] {
            let scene = finale(n, Duration::from_secs(1));
            let Some(&Shape::Text { y, px, text, .. }) = scene.shapes.first() else {
                panic!("点数が出ていない");
            };
            assert_eq!(text, score(n));
            let bottom = y + px / 2;
            for s in &scene.shapes[1..] {
                let (l, t, r, b) = bounds(s).unwrap();
                assert!(t >= bottom, "{n}: 点数に重なった: {s:?}");
                assert!(
                    l >= 0 && r <= WIDTH && b <= HEIGHT,
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

    /// 鉄板は下だけ。上は暗くした鏡のまま。
    #[test]
    fn only_the_lower_part_is_iron() {
        let img = Image::filled(320, 180, (100, 100, 100));
        let p = iron_over(&img);
        assert_eq!(p.at(10, 10), (60, 60, 60));
        assert_ne!(p.at(10, 170), (60, 60, 60));
    }

    /// 舟皿は画面に収まる。たこ焼きが6つ乗っても。
    #[test]
    fn the_boat_fits_on_screen() {
        for s in boat(6, 0.0) {
            let (l, t, r, b) = bounds(&s).unwrap();
            assert!(l >= 0 && t >= 0 && r <= WIDTH && b <= HEIGHT, "{s:?}");
        }
    }
}
