//! カメラ無しで組んだときの `kit::Camera`。何も映らない。

use std::time::Duration;

use anyhow::Result;
use kit::{Camera, Image, BACKGROUND, CAMERA_HEIGHT, CAMERA_WIDTH};

/// 無地の絵を返すだけ。`camera` feature を外したビルド用。
///
/// **カメラの速さを真似て待つ。** 待たずに返すと、絵を待って1周を刻む
/// 遊びが空回りして CPU を食い切る。
///
/// 動きが無いので、カメラで触って遊ぶものはマウスで押して試す
/// （`kit::Press::Point`）。
pub struct Still;

impl Camera for Still {
    fn frame(&mut self) -> Result<Image> {
        std::thread::sleep(Duration::from_millis(33));
        Ok(Image::filled(CAMERA_WIDTH, CAMERA_HEIGHT, BACKGROUND))
    }

    fn rest(&mut self) {}
}
