//! 絵を升目に分けて、升目ごとの明るさを出す。

use kit::Image;

/// 升目ごとの明るさの平均。左上から1行ずつ、`cols * rows` 個。
///
/// **1画素ずつ比べない。** カメラのざらつきは1画素では大きいが、升目で
/// 平均すると消える。
pub fn brightness(img: &Image, cols: usize, rows: usize) -> Vec<u8> {
    let (bw, bh) = ((img.width / cols).max(1), (img.height / rows).max(1));
    let mut out = Vec::with_capacity(cols * rows);
    for gy in 0..rows {
        for gx in 0..cols {
            let mut sum = 0u32;
            let mut n = 0u32;
            for y in (gy * bh)..((gy + 1) * bh).min(img.height) {
                for x in (gx * bw)..((gx + 1) * bw).min(img.width) {
                    let (r, g, b) = img.at(x, y);
                    // 人の目の感じ方に寄せた重み。
                    sum += (r as u32 * 77 + g as u32 * 150 + b as u32 * 29) >> 8;
                    n += 1;
                }
            }
            out.push(sum.checked_div(n).unwrap_or(0) as u8);
        }
    }
    out
}
