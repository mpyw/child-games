//! 何を映すか。鏡に映った自分、たこ焼き器、たこ焼き、舟。
//!
//! 画素は知らない。図形を並べて `kit::Scene` にするところまでで、塗るのは
//! `home` の描き手。鉄板は鏡の絵の下の部分を染めて作る。

use std::sync::OnceLock;
use std::time::Duration;

use kit::{Cap, Image, Rect, Rgb, Scene, Shape, HEIGHT, WIDTH};
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

/// ソース。
const SAUCE: Rgb = (112, 52, 24);

/// 焼け具合の色。0 は焼けていない生地、1 はこんがり。**5段を滑らかに繋ぐ。**
/// 2段で混ぜると、途中がくすんだ灰色を通る。
const TOAST: [(f32, Rgb); 5] = [
    (0.0, (250, 240, 206)),
    (0.3, (247, 224, 165)),
    (0.55, (238, 190, 108)),
    (0.8, (214, 140, 60)),
    (1.0, (186, 104, 38)),
];

/// 焼け具合 `d`（0〜1）の色。
fn toast(d: f32) -> Rgb {
    let d = d.clamp(0.0, 1.0);
    for w in TOAST.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if d <= b {
            return mix(ca, cb, (d - a) / (b - a));
        }
    }
    TOAST[TOAST.len() - 1].1
}

/// 舟の紙（経木）の色。外側、内側、木目。
const FUNE_RIM: Rgb = (198, 162, 112);
const FUNE_FLOOR: Rgb = (238, 214, 170);
const FUNE_GRAIN: Rgb = (204, 170, 122);

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
    shapes.extend(fune(BOAT.0, BOAT.1, 280.0, 120.0, plate.served().min(8), t));
    for h in plate.holes() {
        shapes.extend(ball_in_hole(h, now));
    }
    // **動いた所の点は調整用だけ。** 遊んでいる子どもには、鏡に映った自分が
    // 見えれば足りる。点が散るとたこ焼きが見づらい。
    if motion {
        for (x, y) in flowed.spots() {
            shapes.push(Shape::Circle {
                x,
                y,
                r: 3,
                fill: (255, 255, 220),
            });
        }
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

/// 曲が終わったあと。**上に焼けた数を出し、下に舟に盛ったたこ焼きを並べて
/// 祝う。** 1舟に8つずつ盛る。
///
/// 1つも焼けなかったときも、1舟ぶん盛って同じように祝う。2歳児に「できなかった」を
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
        if served == 0 { 8 } else { served },
        now,
    ));
    Scene {
        shapes,
        caption: format!("── {} ──", score(served)),
        ..Scene::default()
    }
}

/// メニューの札に描く絵。鉄板に、たこ焼きを3つ。生地の面、裏返り途中、焼けた面。
pub fn icon(area: Rect) -> Vec<Shape> {
    let mut v = vec![Shape::RoundRect {
        rect: area.inset(area.h / 12),
        r: area.h / 8,
        fill: IRON,
    }];
    let r = area.h as f32 * 0.16;
    let faces = [Face::Raw(0.8), Face::Turning(0.45), Face::Cooked(1.0)];
    for (&(fx, fy), face) in [(0.27, 0.55), (0.5, 0.55), (0.73, 0.55)].iter().zip(faces) {
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
        v.extend(takoyaki(x, y, r, face, 0.0));
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
        // 生地を流している。生地の面が広がっていく。
        Stage::Pour { at } => {
            let p = (since(at) / POUR.as_secs_f32()).min(1.0);
            takoyaki(x, y, BALL * (0.3 + 0.7 * p), Face::Raw(0.0), t)
        }
        // 片面が焼けていく。鉄板に触れた側から、焼き色が縁を這い上がる。
        Stage::CookA { .. } => {
            let d = h.doneness(now).unwrap_or(0.0);
            let mut v = takoyaki(x, y, BALL, Face::Raw(d), t);
            v.extend(steam(x, y, d * 0.6, t));
            v
        }
        // 片面が焼けた。ぴょこぴょこ跳ね、湯気とキラキラ。
        Stage::ReadyA { at } => {
            let hop = (since(at) * 6.0).sin().abs() * 8.0;
            let mut v = takoyaki(x, y - hop, BALL, Face::Raw(1.0), t);
            v.extend(steam(x, y, 1.0, t));
            v.extend(twinkle(x, y, t, (255, 230, 120)));
            v
        }
        // 裏返っている。**焼けた面が、球の上を転がってこちらへ来る。** 少し跳ねる。
        Stage::Flip { at } => {
            let p = (since(at) / FLIP.as_secs_f32()).min(1.0);
            let lift = (p * std::f32::consts::PI).sin() * 35.0;
            takoyaki(
                x,
                y - lift,
                BALL * (1.0 + 0.06 * lift / 35.0),
                Face::Turning(p),
                t,
            )
        }
        // 反対側が焼けていく。焼けた面が上で、縁にまだ生地の色が残っている。
        Stage::CookB { .. } => {
            let d = h.doneness(now).unwrap_or(0.0);
            let mut v = takoyaki(x, y, BALL, Face::Cooked(d), t);
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
            let mut v = takoyaki(x + dx, y + dy, BALL, Face::Cooked(1.0), t);
            v.extend(twinkle(x, y, t, (255, 245, 170)));
            v
        }
        // 舟皿へ飛ぶ。弧を描いて小さくなる。
        Stage::Flying { at } => {
            let p = (since(at) / FLIGHT.as_secs_f32()).min(1.0);
            let fx = h.x + (BOAT.0 - h.x) * p;
            let fy = h.y + (BOAT.1 - h.y) * p - (p * std::f32::consts::PI).sin() * 200.0;
            takoyaki(fx, fy, BALL * (1.0 - 0.65 * p), Face::Cooked(1.0), t)
        }
        // 落ちた。少し跳ねて戻る。
        Stage::Dropped { at } => {
            let p = (since(at) / DROP.as_secs_f32()).min(1.0);
            let hop = (p * std::f32::consts::PI).sin() * 40.0;
            takoyaki(x, y - hop, BALL, Face::Cooked(1.0), t)
        }
    }
}

/// たこ焼きのどちらの面が上か。
#[derive(Clone, Copy)]
enum Face {
    /// 生地の面が上。片面が焼けていく途中で、値は焼け具合（0〜1）。
    Raw(f32),
    /// 裏返っている途中。値は進み（0〜1）。
    Turning(f32),
    /// 焼けた面が上。反対側が焼けていく途中で、値は焼け具合（0〜1）。
    Cooked(f32),
    /// ソースなどがかかった、できあがり。
    Topped,
}

/// たこ焼き1つ。**光の当たった球（`Shape::Sphere`）で描く。** 焼けた面は
/// 球の一部を塗る `Cap` で表し、裏返すときはその向きを回す。
fn takoyaki(x: f32, y: f32, r: f32, face: Face, t: f32) -> Vec<Shape> {
    let sphere = |fill: Rgb, cap: Option<Cap>| Shape::Sphere {
        x: x as i32,
        y: y as i32,
        r: (r as i32).max(2),
        fill,
        cap,
    };
    match face {
        // 上は生地。焼けた側は鉄板の向こう（奥）にあり、焼けるほど縁から見えてくる。
        //
        // **奥を向けた面は、半球より大きくしないと手前に出てこない。** 手前に
        // 見えるのは奥と反対の半球なので、`size` が 1 を超えたぶんだけ縁に出る。
        Face::Raw(d) => {
            let mut v = vec![sphere(
                mix(toast(0.08), toast(0.3), d * 0.7),
                Some(Cap {
                    fill: toast(0.55 + 0.3 * d),
                    toward: (0.0, 0.0, -1.0),
                    size: 0.95 + 0.55 * d,
                }),
            )];
            v.extend(bits(x, y, r, d, t));
            v
        }
        // 焼けた面（奥）が、下から手前へ転がってくる。
        Face::Turning(p) => {
            let a = p * std::f32::consts::PI;
            vec![sphere(
                mix(toast(0.08), toast(0.3), 0.7),
                Some(Cap {
                    fill: toast(0.85),
                    toward: (0.0, a.sin(), -a.cos()),
                    size: 1.0,
                }),
            )]
        }
        // 焼けた面が上。縁に残る生地の色が、焼けるにつれて消えていく。
        Face::Cooked(d) => {
            let mut v = vec![sphere(
                toast(0.8 + 0.2 * d),
                Some(Cap {
                    fill: toast(0.4 + 0.3 * d),
                    toward: (0.0, 0.0, -1.0),
                    size: 1.5 - 0.55 * d,
                }),
            )];
            v.extend(crust(x, y, r, 0.8 + 0.2 * d));
            v
        }
        Face::Topped => topped(x, y, r),
    }
}

/// 生地の面から覗く具（たこ・ねぎ・紅しょうが）と、焼けるにつれて出る泡。
fn bits(x: f32, y: f32, r: f32, d: f32, t: f32) -> Vec<Shape> {
    let mut v = Vec::new();
    let spots = [
        (-0.3, -0.15, (205, 70, 70)),
        (0.22, 0.12, (205, 70, 70)),
        (0.05, -0.4, (90, 165, 80)),
        (-0.12, 0.32, (90, 165, 80)),
        (0.36, -0.18, (232, 95, 120)),
    ];
    for (i, &(dx, dy, c)) in spots.iter().enumerate() {
        v.push(dot(x + dx * r, y + dy * r, r * 0.09, c));
        if d > 0.2 && (t * 2.0 + i as f32 * 1.7).sin() > 0.6 {
            v.push(dot(
                x - dy * r * 0.8,
                y + dx * r * 0.8,
                r * 0.06,
                (255, 250, 232),
            ));
        }
    }
    v
}

/// 焼けた面のぶつぶつ。少し濃い点を散らす。生地の泡が焼けた跡。
fn crust(x: f32, y: f32, r: f32, d: f32) -> Vec<Shape> {
    let c = mix(toast(d), (110, 55, 20), 0.35);
    [
        (-0.35, -0.1),
        (-0.05, -0.42),
        (0.3, -0.25),
        (0.1, 0.05),
        (-0.25, 0.3),
        (0.35, 0.28),
        (0.0, 0.5),
    ]
    .iter()
    .map(|&(dx, dy)| dot(x + dx * r, y + dy * r, r * 0.05, c))
    .collect()
}

/// 縁の付かない丸い点。角丸を正方形にして角を半分丸めれば、縁の無い丸になる。
fn dot(x: f32, y: f32, r: f32, c: Rgb) -> Shape {
    let r = r.max(1.5);
    Shape::RoundRect {
        rect: Rect {
            x: (x - r) as i32,
            y: (y - r) as i32,
            w: (r * 2.0) as i32,
            h: (r * 2.0) as i32,
        },
        r: r as i32,
        fill: c,
    }
}

/// ソース・マヨネーズ・青のり・かつお節がかかったたこ焼き。舟に盛るのに使う。
///
/// ソースは球の上を塗る `Cap` で描く。球と一緒に光が当たるので、てかって見える。
fn topped(x: f32, y: f32, r: f32) -> Vec<Shape> {
    let mut v = vec![Shape::Sphere {
        x: x as i32,
        y: y as i32,
        r: (r as i32).max(2),
        fill: toast(1.0),
        cap: Some(Cap {
            fill: SAUCE,
            toward: (0.0, -0.55, 0.83),
            size: 0.72,
        }),
    }];
    // マヨネーズ。波打つ線を2本、点を並べて描く。
    let mayo = (250, 246, 228);
    for (row, dy) in [-0.45f32, -0.12].iter().enumerate() {
        for k in 0..15 {
            let u = k as f32 / 14.0;
            let px = x + (u - 0.5) * r * 1.2;
            let py = y + dy * r + (u * 9.0 + row as f32).sin() * r * 0.1;
            v.push(dot(px, py, r * 0.07, mayo));
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
        v.push(dot(x + dx * r, y + dy * r, r * 0.05, (70, 140, 50)));
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

/// 経木（木目のある紙）の舟。`(cx, cy)` が真ん中、`w`・`h` が大きさ。たこ焼きを
/// 2列に `n` 個（8つまで）盛る。
///
/// 上から少し見下ろした形。外側は台形（奥が広く、手前がすぼまる）で、内側の
/// 底に木目の筋を波打たせて入れる。
fn fune(cx: f32, cy: f32, w: f32, h: f32, n: usize, _t: f32) -> Vec<Shape> {
    let (x0, x1) = (cx - w / 2.0, cx + w / 2.0);
    let (y0, y1) = (cy - h / 2.0, cy + h / 2.0);
    let slant = w * 0.07;
    let p = |x: f32, y: f32| (x as i32, y as i32);
    let mut v = vec![
        // 外側。真ん中の長方形と、両脇の斜めの側面。
        Shape::RoundRect {
            rect: Rect {
                x: (x0 + slant) as i32,
                y: y0 as i32,
                w: (w - slant * 2.0) as i32,
                h: h as i32,
            },
            r: (h * 0.08) as i32,
            fill: FUNE_RIM,
        },
        Shape::Triangle {
            points: [p(x0, y0), p(x0 + slant + 2.0, y0), p(x0 + slant + 2.0, y1)],
            fill: FUNE_RIM,
        },
        Shape::Triangle {
            points: [p(x1, y0), p(x1 - slant - 2.0, y0), p(x1 - slant - 2.0, y1)],
            fill: FUNE_RIM,
        },
    ];
    // 内側の底。
    let (fx0, fx1) = (x0 + slant * 1.9, x1 - slant * 1.9);
    let (fy0, fy1) = (y0 + h * 0.13, y1 - h * 0.1);
    v.push(Shape::RoundRect {
        rect: Rect {
            x: fx0 as i32,
            y: fy0 as i32,
            w: (fx1 - fx0) as i32,
            h: (fy1 - fy0) as i32,
        },
        r: (h * 0.05) as i32,
        fill: FUNE_FLOOR,
    });
    // 木目。細い筋を短く刻んで、少しずつ上下にずらして並べる。
    let lines = 5;
    let segs = 14;
    for l in 0..lines {
        let base = fy0 + (fy1 - fy0) * (l as f32 + 0.5) / lines as f32;
        let seg = (fx1 - fx0) / segs as f32;
        for k in 0..segs {
            let wave = ((k as f32 * 0.7 + l as f32 * 1.9).sin() + (k as f32 * 0.23).cos() * 0.5)
                * h
                * 0.025;
            v.push(Shape::RoundRect {
                rect: Rect {
                    x: (fx0 + k as f32 * seg) as i32,
                    y: (base + wave) as i32,
                    w: (seg + 1.0) as i32,
                    h: (h * 0.02).max(1.0) as i32,
                },
                r: 0,
                fill: FUNE_GRAIN,
            });
        }
    }
    // たこ焼き。奥の列を先に描く（手前の列が重なる）。
    let n = n.min(8);
    let r = ((fx1 - fx0) / 9.5).min((fy1 - fy0) / 3.7);
    for i in 0..n {
        let (row, col) = (i / 4, i % 4);
        let bx = fx0 + (fx1 - fx0) * (col as f32 + 0.5) / 4.0;
        let by = fy0 + (fy1 - fy0) * if row == 0 { 0.3 } else { 0.7 };
        v.extend(takoyaki(bx, by, r, Face::Topped, 0.0));
    }
    v
}

/// `area` に、たこ焼きを8つずつ盛った舟を並べる。舟が多ければ小さくする。
fn lineup(area: Rect, n: usize, now: Duration) -> Vec<Shape> {
    let n = n.max(1);
    let boats = n.div_ceil(8);
    let cols = (1..=boats)
        .find(|c| c * c * 2 >= boats)
        .unwrap_or(boats)
        .max(1);
    let rows = boats.div_ceil(cols);
    let (cw, ch) = (area.w as f32 / cols as f32, area.h as f32 / rows as f32);
    // 舟は横長（幅:高さ = 7:3）。区画の 8 割に収める。
    let w = (cw * 0.85).min(ch * 0.8 * 7.0 / 3.0);
    let h = w * 3.0 / 7.0;
    let t = now.as_secs_f32();
    (0..boats)
        .flat_map(|b| {
            let bob = (t * 2.0 + b as f32 * 0.7).sin() * h * 0.03;
            fune(
                area.x as f32 + (b % cols) as f32 * cw + cw / 2.0,
                area.y as f32 + (b / cols) as f32 * ch + ch / 2.0 + bob,
                w,
                h,
                (n - b * 8).min(8),
                t,
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
            Shape::Circle { x, y, r, .. } | Shape::Sphere { x, y, r, .. } => {
                Some((x - r, y - r, x + r, y + r))
            }
            Shape::RoundRect { rect, .. } => {
                Some((rect.x, rect.y, rect.x + rect.w, rect.y + rect.h))
            }
            Shape::Triangle { points, .. } => {
                let xs = points.map(|p| p.0);
                let ys = points.map(|p| p.1);
                Some((
                    *xs.iter().min().unwrap(),
                    *ys.iter().min().unwrap(),
                    *xs.iter().max().unwrap(),
                    *ys.iter().max().unwrap(),
                ))
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

    /// **焼け具合の色は、焼けるほど暗くなる。** 途中で明るさが戻らない。
    /// 青みも足さない（灰色にくすまない）。
    #[test]
    fn toasting_only_darkens() {
        let lum = |c: Rgb| c.0 as u32 * 3 + c.1 as u32 * 6 + c.2 as u32;
        let mut prev = toast(0.0);
        for i in 1..=100 {
            let c = toast(i as f32 / 100.0);
            assert!(lum(c) <= lum(prev), "{i}: {prev:?} → {c:?}");
            assert!(c.2 <= prev.2, "{i}: 青みが増えた {prev:?} → {c:?}");
            assert!(
                c.0 as i32 > c.2 as i32 + 100 || i < 50,
                "{i}: 焼け色になっていない {c:?}"
            );
            prev = c;
        }
    }

    /// 舟に盛る数は8つまで。多ければ舟を足す。
    #[test]
    fn boats_hold_eight_each() {
        let spheres = |v: &[Shape]| {
            v.iter()
                .filter(|s| matches!(s, Shape::Sphere { .. }))
                .count()
        };
        assert_eq!(spheres(&fune(640.0, 360.0, 280.0, 120.0, 20, 0.0)), 8);
        let all = lineup(Rect::SCREEN, 20, Duration::ZERO);
        assert_eq!(spheres(&all), 20);
    }

    /// 鉄板は下だけ。上は暗くした鏡のまま。
    #[test]
    fn only_the_lower_part_is_iron() {
        let img = Image::filled(320, 180, (100, 100, 100));
        let p = iron_over(&img);
        assert_eq!(p.at(10, 10), (60, 60, 60));
        assert_ne!(p.at(10, 170), (60, 60, 60));
    }

    /// 右上の舟は画面に収まる。たこ焼きが8つ乗っても。
    #[test]
    fn the_boat_fits_on_screen() {
        for s in fune(BOAT.0, BOAT.1, 280.0, 120.0, 8, 0.0) {
            let (l, t, r, b) = bounds(&s).unwrap();
            assert!(l >= 0 && t >= 0 && r <= WIDTH && b <= HEIGHT, "{s:?}");
        }
    }
}
