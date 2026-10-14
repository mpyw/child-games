//! 何を映すか。鏡に映った自分と、風船。
//!
//! 画素は知らない。図形を並べて `kit::Scene` にするところまでで、塗るのは
//! `home` の描き手。

use std::sync::OnceLock;
use std::time::Duration;

use kit::{Image, Rect, Rgb, Scene, Shape, HEIGHT, WIDTH};
use vision::Moved;

use crate::field::{Field, BURST, COLORS};

/// 鏡の暗さ。**風船が主役。** 明るいままだと風船が背景に紛れる。
/// 暗くしすぎると自分が見えず、どこを触ればいいか分からない。
const MIRROR: f32 = 0.55;

/// 遊んでいる間の場面。
pub fn playing(mirror: &Image, field: &Field, moved: &Moved, now: Duration) -> Scene {
    let mut shapes = Vec::new();
    // **動いた所に小さな光を出す。** 自分が触っていることが見える。
    for (x, y) in moved.spots() {
        shapes.push(Shape::Circle {
            x,
            y,
            r: 4,
            fill: (255, 255, 230),
        });
    }
    let r = field.radius(now);
    for b in field.balloons() {
        shapes.extend(balloon(b.x as i32, b.y as i32, r as i32, b.color));
    }
    for p in field.bursts() {
        let t = now.saturating_sub(p.at).as_secs_f32() / BURST.as_secs_f32();
        shapes.extend(burst(p.x, p.y, p.r, p.color, t.min(1.0)));
    }
    Scene {
        image: Some(mirror.shaded(MIRROR)),
        shapes,
        caption: format!(
            "── ふうせん {}こ / わった {}こ ──",
            field.balloons().len(),
            field.popped().len()
        ),
    }
}

/// 点数に使う字。`Game::glyphs` で `home` に渡し、フォントに入っているかを見させる。
pub const GLYPHS: &str = "0123456789こ わったね！";

/// 点数の上限。1分で割れるのはせいぜい百数十なので、ここで頭打ちにする。
const MAX_SCORE: usize = 999;

/// 割った数を書いた字。「12こ わったね！」。
///
/// **一度だけ作って持っておく。** `Shape::Text` は `&'static str` しか
/// 持てない。毎フレーム作って手放せない文字列にすると、遊ぶほど漏れる。
pub fn score(n: usize) -> &'static str {
    static TEXTS: OnceLock<Vec<String>> = OnceLock::new();
    let texts = TEXTS.get_or_init(|| {
        (0..=MAX_SCORE)
            .map(|n| format!("{n}こ わったね！"))
            .collect()
    });
    &texts[n.min(MAX_SCORE)]
}

/// 曲が終わったあと。**上に割った数を出し、下に割った風船を並べて祝う。**
///
/// 数字は親が読む。子どもには並んだ風船が見える。1つも割れなかったときも、
/// 色を全部並べて同じように祝う。2歳児に「できなかった」を見せる理由は無い。
pub fn finale(popped: &[Rgb], now: Duration) -> Scene {
    let colors: Vec<Rgb> = if popped.is_empty() {
        COLORS.to_vec()
    } else {
        popped.to_vec()
    };
    let top = HEIGHT * 28 / 100;
    let mut shapes = vec![Shape::Text {
        x: WIDTH / 2,
        y: HEIGHT * 14 / 100,
        px: HEIGHT / 8,
        text: score(popped.len()),
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
        caption: format!("── {} ──", score(popped.len())),
        ..Scene::default()
    }
}

/// メニューの札に描く絵。風船を3つ。
pub fn icon(area: Rect) -> Vec<Shape> {
    let r = (area.h as f32 * 0.22) as i32;
    let spots = [(0.25, 0.55), (0.5, 0.38), (0.75, 0.55)];
    spots
        .iter()
        .zip(COLORS.iter().step_by(3))
        .flat_map(|(&(fx, fy), &c)| {
            balloon(
                area.x + (area.w as f32 * fx) as i32,
                area.y + (area.h as f32 * fy) as i32,
                r,
                c,
            )
        })
        .collect()
}

/// `area` に風船を並べる。数が多ければ小さくする。ゆっくり揺らす。
fn lineup(area: Rect, colors: &[Rgb], now: Duration) -> Vec<Shape> {
    let n = colors.len().max(1);
    // 横長の画面に収まるよう、列を多めに取る。
    let cols = (1..=n).find(|c| c * c * 9 >= n * 16).unwrap_or(n).max(1);
    let rows = n.div_ceil(cols);
    let (cw, ch) = (area.w / cols as i32, area.h / rows as i32);
    let r = ((cw.min(ch) as f32) * 0.3) as i32;
    colors
        .iter()
        .enumerate()
        .flat_map(|(i, &c)| {
            let bob = ((now.as_secs_f32() * 2.0 + i as f32 * 0.7).sin() * r as f32 * 0.12) as i32;
            balloon(
                area.x + (i % cols) as i32 * cw + cw / 2,
                area.y + (i / cols) as i32 * ch + ch * 2 / 5 + bob,
                r,
                c,
            )
        })
        .collect()
}

/// 風船1つ。紐、結び目、本体、光の順に重ねる。
fn balloon(x: i32, y: i32, r: i32, c: Rgb) -> Vec<Shape> {
    let thread = (r / 30).max(1);
    vec![
        Shape::RoundRect {
            rect: Rect {
                x: x - thread,
                y: y + r,
                w: thread * 2,
                h: r * 3 / 2,
            },
            r: thread,
            fill: (225, 225, 225),
        },
        Shape::Circle {
            x,
            y: y + r,
            r: (r / 7).max(2),
            fill: c,
        },
        Shape::Circle { x, y, r, fill: c },
        Shape::Circle {
            x: x - r * 2 / 5,
            y: y - r * 2 / 5,
            r: (r / 6).max(2),
            fill: lighten(c),
        },
    ]
}

/// 弾けた跡。かけらが外へ散って、小さくなって消える。
fn burst(x: f32, y: f32, r: f32, c: Rgb, t: f32) -> Vec<Shape> {
    const PIECES: usize = 10;
    (0..PIECES)
        .map(|i| {
            let a = i as f32 / PIECES as f32 * std::f32::consts::TAU;
            let d = r * (0.4 + t * 1.1);
            Shape::Circle {
                x: (x + a.cos() * d) as i32,
                y: (y + a.sin() * d) as i32,
                r: ((r * 0.2 * (1.0 - t)) as i32).max(1),
                fill: c,
            }
        })
        .collect()
}

fn lighten(c: Rgb) -> Rgb {
    let m = |v: u8| v + (255 - v) / 2;
    (m(c.0), m(c.1), m(c.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **札に縮めても、はみ出さない。** 紐は下へ垂れるので本体だけ見る。
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

    /// 割った数がどれだけ多くても、並べた風船は画面に収まる。
    #[test]
    fn the_lineup_fits_on_screen() {
        for n in [0, 1, 7, 30, 120] {
            let popped = vec![(255, 0, 0); n];
            for s in finale(&popped, Duration::from_secs(1)).shapes {
                if let Shape::Circle { x, y, r, .. } = s {
                    assert!(x - r >= 0 && x + r <= WIDTH, "{n}: {s:?}");
                    assert!(y - r >= 0 && y + r <= HEIGHT, "{n}: {s:?}");
                }
            }
        }
    }

    /// 1つも割れなくても、お祝いには風船が並ぶ。
    #[test]
    fn nothing_popped_is_still_a_celebration() {
        let circles = finale(&[], Duration::ZERO)
            .shapes
            .iter()
            .filter(|s| matches!(s, Shape::Circle { .. }))
            .count();
        assert!(circles > 0);
    }

    /// **割った数が上に出る。** 並べた風船と重ならない。
    #[test]
    fn the_score_is_shown_above_the_lineup() {
        let scene = finale(&[(255, 0, 0); 12], Duration::ZERO);
        let Some(&Shape::Text { y, px, text, .. }) = scene.shapes.first() else {
            panic!("点数が出ていない");
        };
        assert_eq!(text, "12こ わったね！");
        let bottom = y + px / 2;
        for s in &scene.shapes[1..] {
            if let Shape::Circle { y: cy, r, .. } = *s {
                assert!(cy - r >= bottom, "風船が点数に重なった: {s:?}");
            }
        }
    }

    /// 点数の字は、`GLYPHS` に入っているものだけで書ける。
    #[test]
    fn the_score_uses_only_the_listed_glyphs() {
        for n in [0, 7, 42, 999, 5000] {
            for ch in score(n).chars() {
                assert!(GLYPHS.contains(ch), "{n}: 「{ch}」が GLYPHS に無い");
            }
        }
        assert_eq!(score(5000), score(999), "頭打ちになっていない");
    }
}
