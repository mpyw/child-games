//! テスト用の絵。**遊びのテストからも使う**ので、`cfg(test)` にしない。

use kit::{Image, CAMERA_HEIGHT, CAMERA_WIDTH, WIDTH};

/// 背景の灰色。
pub const GREY: (u8, u8, u8) = (80, 80, 80);

/// 一面灰色の絵。
pub fn grey() -> Image {
    Image::filled(CAMERA_WIDTH, CAMERA_HEIGHT, GREY)
}

/// 灰色の絵の、キャンバスで `(x, y)` を中心に半径 `r` の丸だけ白くしたもの。
pub fn blob(x: i32, y: i32, r: i32) -> Image {
    blobs(&[(x, y, r)])
}

/// 白い丸をいくつか置いた絵。座標はキャンバス。
///
/// **縁をぼかす。** くっきりした丸だと、ずらして比べたときに升目の中の
/// 明るさが段で変わり、動いた向きの手掛かりが角にしか残らない。カメラに
/// 映る手も縁はなだらかなので、そちらに寄せる。
pub fn blobs(spots: &[(i32, i32, i32)]) -> Image {
    let s = CAMERA_WIDTH as f32 / WIDTH as f32;
    let px = (0..CAMERA_WIDTH * CAMERA_HEIGHT)
        .map(|i| {
            let (px, py) = ((i % CAMERA_WIDTH) as f32, (i / CAMERA_WIDTH) as f32);
            let mut v = GREY.0 as f32;
            for &(x, y, r) in spots {
                let (bx, by, br) = (x as f32 * s, y as f32 * s, r as f32 * s);
                let d = ((px - bx).powi(2) + (py - by).powi(2)).sqrt();
                // 半径の 7 割までは真っ白、そこから縁へ向けて灰色に戻す。
                let t = ((br - d) / (br * 0.3)).clamp(0.0, 1.0);
                v = v.max(GREY.0 as f32 + (250.0 - GREY.0 as f32) * t);
            }
            let v = v as u8;
            (v, v, v)
        })
        .collect::<Vec<_>>();
    Image {
        width: CAMERA_WIDTH,
        height: CAMERA_HEIGHT,
        pixels: px.into(),
    }
}
