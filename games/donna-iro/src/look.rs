//! 何を映すか。丸を並べるだけ。
//!
//! 画素は知らない。図形を並べて `kit::Scene` にするところまでで、塗るのは
//! `home` の描き手。

use kit::{Rect, Scene, Shape, HEIGHT, WIDTH};
use strum::VariantArray;

use crate::color::Color;

/// 全色を並べる。質問・区切りのとき。
///
/// 並び順を受け取るのは、フィナーレで位置ごとに色を入れ替えるため。
/// 通常は宣言順（`Color::VARIANTS`）。
pub fn palette(order: &[Color]) -> Scene {
    let caption = if order == Color::VARIANTS {
        "── ぜんぶ ──".to_string()
    } else {
        let names: Vec<&str> = order.iter().map(|c| c.name()).collect();
        format!("── {} ──", names.join(" "))
    };
    Scene {
        shapes: grid(Rect::SCREEN, order),
        caption,
        ..Scene::default()
    }
}

/// 1色だけ大きく。その色のフレーズを歌っている間。
pub fn single(c: Color) -> Scene {
    Scene {
        shapes: vec![Shape::Circle {
            x: WIDTH / 2,
            y: HEIGHT / 2,
            r: (HEIGHT as f32 * 0.38) as i32,
            fill: c.rgb(),
        }],
        caption: format!("● {}", c.name()),
        ..Scene::default()
    }
}

/// `area` に全色を4列で並べる。テレビで見たとき、12色なら4列×3行が
/// 一番収まりがいい。メニューの札にも同じ並びを縮めて使う。
pub fn grid(area: Rect, order: &[Color]) -> Vec<Shape> {
    let cols = 4usize;
    let rows = order.len().div_ceil(cols).max(1);
    let cw = area.w / cols as i32;
    let ch = area.h / rows as i32;
    let r = (cw.min(ch) as f32 * 0.36) as i32;
    order
        .iter()
        .enumerate()
        .map(|(i, c)| Shape::Circle {
            x: area.x + (i % cols) as i32 * cw + cw / 2,
            y: area.y + (i / cols) as i32 * ch + ch / 2,
            r,
            fill: c.rgb(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **札に縮めても、はみ出さない。** メニューで隣の札に食い込む。
    #[test]
    fn the_grid_stays_inside_its_area() {
        let area = Rect {
            x: 100,
            y: 50,
            w: 400,
            h: 200,
        };
        for s in grid(area, Color::VARIANTS) {
            let Shape::Circle { x, y, r, .. } = s else {
                panic!("丸以外を描いた");
            };
            assert!(x - r >= area.x && x + r <= area.x + area.w, "{s:?}");
            assert!(y - r >= area.y && y + r <= area.y + area.h, "{s:?}");
        }
    }
}
