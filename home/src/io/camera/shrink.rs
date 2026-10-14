//! 撮った絵を、遊びに渡す大きさと向きに整える。

use kit::{Image, Rgb, CAMERA_HEIGHT, CAMERA_WIDTH};

/// `width` × `height` の絵から、真ん中を 16:9 に切り出して
/// `CAMERA_WIDTH` × `CAMERA_HEIGHT` に縮め、**左右を反転する。**
///
/// `at(x, y)` は元の絵の画素。カメラの形式ごとに読み方が違うので、
/// 読むのは呼ぶ側に任せる。
///
/// **縮めるときに 2×2 点の平均を取る。** 1点だけ拾うと、カメラの
/// ざらつきがそのまま残り、遊びの側で「動いた」に見える。
///
/// 反転するのは、テレビを鏡として見せるため。子どもが右手を上げたら
/// 画面の右が動かないと、どこを触ればいいか分からない。
pub fn shrink(width: usize, height: usize, at: impl Fn(usize, usize) -> Rgb) -> Image {
    let (w, h) = (CAMERA_WIDTH, CAMERA_HEIGHT);
    // 16:9 より横長なら左右を、縦長なら上下を落とす。
    let (cw, ch) = if width * h > height * w {
        (height * w / h, height)
    } else {
        (width, width * h / w)
    };
    let (x0, y0) = ((width - cw) / 2, (height - ch) / 2);

    let mut pixels = Vec::with_capacity(w * h);
    for ty in 0..h {
        for tx in 0..w {
            // 鏡にするので、右端の画素を左端に持ってくる。
            let sx = w - 1 - tx;
            let mut sum = (0u32, 0u32, 0u32);
            for (fx, fy) in [(1, 1), (3, 1), (1, 3), (3, 3)] {
                // 区画を4等分した点。区画の幅が1画素未満でも端を越えない。
                let x = x0 + ((sx * 4 + fx) * cw / (w * 4)).min(cw - 1);
                let y = y0 + ((ty * 4 + fy) * ch / (h * 4)).min(ch - 1);
                let c = at(x, y);
                sum = (sum.0 + c.0 as u32, sum.1 + c.1 as u32, sum.2 + c.2 as u32);
            }
            pixels.push(((sum.0 / 4) as u8, (sum.1 / 4) as u8, (sum.2 / 4) as u8));
        }
    }
    Image {
        width: w,
        height: h,
        pixels: pixels.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 左半分が白、右半分が黒の絵。
    fn halves(width: usize, height: usize) -> Image {
        shrink(width, height, |x, _| {
            if x < width / 2 {
                (255, 255, 255)
            } else {
                (0, 0, 0)
            }
        })
    }

    /// **左右が入れ替わる。** 鏡になっていないと、触る場所が逆になる。
    #[test]
    fn the_picture_is_a_mirror() {
        let img = halves(640, 360);
        assert_eq!((img.width, img.height), (CAMERA_WIDTH, CAMERA_HEIGHT));
        assert_eq!(img.at(0, 90), (0, 0, 0), "左端に元の右側が来ていない");
        assert_eq!(img.at(CAMERA_WIDTH - 1, 90), (255, 255, 255));
    }

    /// 4:3 のカメラは上下を落として 16:9 にする。左右はそのまま残る。
    #[test]
    fn a_four_by_three_camera_is_cropped_top_and_bottom() {
        // 上の 1/8 だけ赤い。16:9 に切ると上下 1/8 ずつ落ちるので消える。
        let img = shrink(
            640,
            480,
            |_, y| if y < 60 { (255, 0, 0) } else { (0, 0, 255) },
        );
        assert!(
            img.pixels.iter().all(|&c| c == (0, 0, 255)),
            "上下を落としていない"
        );
        // 左右は落とさない。
        let img = halves(640, 480);
        assert_eq!(img.at(0, 90), (0, 0, 0));
        assert_eq!(img.at(CAMERA_WIDTH - 1, 90), (255, 255, 255));
    }

    /// 横長すぎるカメラは左右を落とす。
    #[test]
    fn a_wide_camera_is_cropped_left_and_right() {
        // 21:9。左右の端だけ緑。
        let img = shrink(840, 360, |x, _| {
            if !(100..740).contains(&x) {
                (0, 255, 0)
            } else {
                (9, 9, 9)
            }
        });
        assert!(
            img.pixels.iter().all(|&c| c == (9, 9, 9)),
            "左右を落としていない"
        );
    }

    /// 縮める先より小さいカメラでも落ちない。
    #[test]
    fn a_tiny_camera_does_not_panic() {
        let img = shrink(16, 9, |x, y| ((x * 10) as u8, (y * 10) as u8, 0));
        assert_eq!(img.pixels.len(), CAMERA_WIDTH * CAMERA_HEIGHT);
    }
}
