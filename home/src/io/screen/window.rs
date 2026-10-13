//! ウィンドウに描く。
//!
//! 遊びとメニューが組んだ図形（`kit::Scene`）を、そのまま画素に落とす。
//! **何の場面かは知らない。** 丸を並べているのが色当てなのかメニューなのか、
//! ここからは見えない。
//!
//! ターミナルと違って実解像度で描けるので、丸はそのまま丸に出る。
//!
//! macOS ではウィンドウをメインスレッドに置く必要があり、しかも
//! `cpal::Stream` も `rodio::OutputStream` も `!Send` なので、
//! **ゲーム側をワーカースレッドに出してメインスレッドは描画に専念**する。
//! 再生や認識で固まらなくなる副次効果もある。

use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use kit::{Press, Rect, Rgb, Scene, Screen, Shape, BACKGROUND, HEIGHT, WIDTH};
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};

/// 画面の更新間隔。音のほうが主役なので60fpsも要らない。
const FPS: u64 = 30;

/// バッファの大きさ。キャンバスと同じ。
const W: usize = WIDTH as usize;
const H: usize = HEIGHT as usize;

/// ゲーム側が持つ手元の口。実際の描画はメインスレッドがやる。
pub struct Remote(pub Sender<Scene>);

impl Screen for Remote {
    fn show(&mut self, scene: Scene) {
        // 受け手が閉じていても、ゲームを止める理由にはならない。
        let _ = self.0.send(scene);
    }
}

/// キーボードのキーと、その意味。
///
/// 数字は番号で選ぶ（メニュー）。Backspace は「戻る」で、テレビの前に
/// リモコンが無いときの代わり。Escape はウィンドウごと閉じる（下の
/// ループが見ている）ので、ここには入れない。
const KEYS: &[(Key, Press)] = &[
    (Key::Space, Press::Ok),
    (Key::Enter, Press::Ok),
    (Key::Backspace, Press::Back),
    (Key::Up, Press::Up),
    (Key::Down, Press::Down),
    (Key::Left, Press::Left),
    (Key::Right, Press::Right),
    (Key::Key1, Press::Pick(0)),
    (Key::Key2, Press::Pick(1)),
    (Key::Key3, Press::Pick(2)),
    (Key::Key4, Press::Pick(3)),
    (Key::Key5, Press::Pick(4)),
    (Key::Key6, Press::Pick(5)),
    (Key::Key7, Press::Pick(6)),
    (Key::Key8, Press::Pick(7)),
    (Key::Key9, Press::Pick(8)),
];

/// メインスレッドで回す。ウィンドウが閉じられたら返る。
///
/// `presses` は押されたものを送る口。テレビの前にキーボードもマウスも
/// 無い前提なので、本命は CEC のリモコン（`control::cec` が同じ口に流す）。
pub fn run(rx: Receiver<Scene>, presses: Sender<Press>) -> Result<()> {
    let mut window = Window::new(
        "どんないろがすき",
        W,
        H,
        WindowOptions {
            resize: true,
            scale_mode: minifb::ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )
    .context("ウィンドウを開けない")?;
    window.set_target_fps(FPS as usize);
    // **矢印を消す。** テレビの前にマウスは無いので、置き場所によっては
    // 画面の真ん中に矢印が居座る。色を見る邪魔になるし、2歳児には何なのか
    // 分からない。
    //
    // クリックは消しても受けられる。メニューの札は大きく、「もう1回」は
    // どこを押しても同じ合図になるので、狙うための矢印がそもそも要らない。
    window.set_cursor_visibility(false);

    let mut buf = vec![0u32; W * H];
    let mut scene = Scene::default();
    let mut was_down = (false, false);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // 溜まっているぶんは捨てて最新だけ描く。
        loop {
            match rx.try_recv() {
                Ok(s) => scene = s,
                Err(TryRecvError::Empty) => break,
                // ゲーム側が終わった（入力が絶えた、または落ちた）。
                // ウィンドウだけ残っても意味がないので畳む。
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }

        // 待っていないときに送ったぶんは受け手が捨てる。ここでは
        // 何を待っているかを知らないので、素通しでよい。
        for (key, press) in KEYS {
            // KeyRepeat::No で押した瞬間だけ拾う。
            if window.is_key_pressed(*key, KeyRepeat::No) {
                let _ = presses.send(*press);
            }
        }

        // クリックは押した瞬間だけ拾う。get_mouse_down は押している間ずっと
        // 真なので、そのまま流すと 30fps ぶんの連打になる。
        let down = (
            window.get_mouse_down(MouseButton::Left),
            window.get_mouse_down(MouseButton::Right),
        );
        if down.0 && !was_down.0 {
            // **位置はキャンバスに直して送る。** 何が描いてあるかを知って
            // いるのは受け手なので、当たり判定は向こうでする。
            let at = window
                .get_unscaled_mouse_pos(MouseMode::Pass)
                .and_then(|pos| to_canvas(pos, window.get_size()));
            if let Some((x, y)) = at {
                let _ = presses.send(Press::Point(x, y));
            }
        }
        // 右クリックは「戻る」。マウスしか無いときの逃げ道。
        if down.1 && !was_down.1 {
            let _ = presses.send(Press::Back);
        }
        was_down = down;

        paint(&mut buf, &scene);
        window.update_with_buffer(&buf, W, H)?;
    }
    Ok(())
}

/// ウィンドウ上の位置を、キャンバスの座標に直す。
///
/// **縦横比を保って伸ばしている**（`AspectRatioStretch`）ので、余った側に
/// 黒い帯が入る。帯の上を押したら `None`。位置と大きさはどちらも拡大率で
/// 割る前の値を渡すこと（単位を揃えるため）。
fn to_canvas(pos: (f32, f32), size: (usize, usize)) -> Option<(i32, i32)> {
    let (ww, wh) = (size.0 as f32, size.1 as f32);
    if ww <= 0.0 || wh <= 0.0 {
        return None;
    }
    let scale = (ww / WIDTH as f32).min(wh / HEIGHT as f32);
    let (ox, oy) = (
        (ww - WIDTH as f32 * scale) / 2.0,
        (wh - HEIGHT as f32 * scale) / 2.0,
    );
    let x = ((pos.0 - ox) / scale).floor() as i32;
    let y = ((pos.1 - oy) / scale).floor() as i32;
    Rect::SCREEN.contains(x, y).then_some((x, y))
}

fn rgb(c: Rgb) -> u32 {
    ((c.0 as u32) << 16) | ((c.1 as u32) << 8) | c.2 as u32
}

fn paint(buf: &mut [u32], scene: &Scene) {
    buf.fill(rgb(BACKGROUND));
    for shape in &scene.shapes {
        match *shape {
            Shape::Circle { x, y, r, fill } => circle(buf, x, y, r, fill),
            Shape::RoundRect { rect, r, fill } => round_rect(buf, rect, r, fill),
            Shape::Text {
                x,
                y,
                px,
                text,
                fill,
            } => self::text(buf, x, y, px as f32, text, fill),
            Shape::Button { x, y, px, text } => button(buf, x, y, px as f32, text),
        }
    }
}

/// 押せることを示すボタン。字の幅から大きさを決める。
///
/// **暗い縁を先に敷く。** 後ろの白と黒の丸がボタンの端から覗いて、
/// どこまでがボタンなのか分からなくなるため。
fn button(buf: &mut [u32], cx: i32, cy: i32, px: f32, label: &str) {
    let w = text_width(label, px);
    let (bw, bh) = (w + (px * 1.6) as i32, (px * 2.0) as i32);
    let r = (px * 0.4) as i32;
    let face = Rect {
        x: cx - bw / 2,
        y: cy - bh / 2,
        w: bw,
        h: bh,
    };
    let pad = (px * 0.07) as i32;
    round_rect(buf, face.inset(-pad), r + pad, (18, 18, 18));
    round_rect(buf, face, r, (245, 245, 245));
    text(buf, cx, cy, px, label, (26, 26, 26));
}

/// 角の丸い長方形。
fn round_rect(buf: &mut [u32], rect: Rect, r: i32, c: Rgb) {
    let fill = rgb(c);
    let (x0, y0, x1, y1) = (rect.x, rect.y, rect.x + rect.w, rect.y + rect.h);
    // 角の丸が辺より大きいと clamp の上下が入れ替わって落ちる。
    let r = r.clamp(0, rect.w.min(rect.h) / 2);
    for y in y0.max(0)..y1.min(HEIGHT) {
        for x in x0.max(0)..x1.min(WIDTH) {
            // 角の内側に丸を1つ置いて、そこから外れた画素だけ落とす。
            let nx = x.clamp(x0 + r, x1 - r);
            let ny = y.clamp(y0 + r, y1 - r);
            let (dx, dy) = (x - nx, y - ny);
            if dx * dx + dy * dy > r * r {
                continue;
            }
            buf[y as usize * W + x as usize] = fill;
        }
    }
}

/// 埋め込んだサブセットフォント。画面に出す字しか入っていない。
/// 出す文字を増やすときは `fonts/README.md` の手順で作り直すこと。
const FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/ui.ttf"));

/// 解析は一度きり。毎フレームやると読み込みのほうが描画より重くなる。
fn font() -> &'static fontdue::Font {
    static PARSED: OnceLock<fontdue::Font> = OnceLock::new();
    PARSED.get_or_init(|| {
        fontdue::Font::from_bytes(FONT, fontdue::FontSettings::default())
            .expect("埋め込んだフォントを読めない")
    })
}

fn text_width(s: &str, px: f32) -> i32 {
    let f = font();
    s.chars()
        .map(|c| f.metrics(c, px).advance_width)
        .sum::<f32>() as i32
}

/// 文字列を `(cx, cy)` を中心に描く。
///
/// 下地の色は問わない。ラスタライザが返す被覆率で混ぜるので、丸の上に
/// 重ねても縁がぎざつかない。
fn text(buf: &mut [u32], cx: i32, cy: i32, px: f32, s: &str, c: Rgb) {
    let f = font();
    // ascent は上、descent は下（負）。字面の中心が cy に来るようずらす。
    //
    // **メトリクスが無くても描く。** ここはメインスレッドなので、落ちると
    // 遊んでいる最中にウィンドウごと消える。字がすこし上下にずれるだけの
    // ことに、その値打ちは無い。
    let baseline = cy
        + match f.horizontal_line_metrics(px) {
            Some(line) => ((line.ascent + line.descent) / 2.0) as i32,
            None => (px * 0.3) as i32,
        };

    let mut pen = cx - text_width(s, px) / 2;
    for ch in s.chars() {
        let (m, cov) = f.rasterize(ch, px);
        // ymin はベースラインから見た下端。画面は下が正なので符号が返る。
        let x0 = pen + m.xmin;
        let y0 = baseline - m.ymin - m.height as i32;
        for gy in 0..m.height {
            for gx in 0..m.width {
                let a = cov[gy * m.width + gx];
                if a == 0 {
                    continue;
                }
                let (x, y) = (x0 + gx as i32, y0 + gy as i32);
                if !Rect::SCREEN.contains(x, y) {
                    continue;
                }
                let i = y as usize * W + x as usize;
                let under = buf[i];
                let dst = ((under >> 16) as u8, (under >> 8) as u8, under as u8);
                buf[i] = rgb(mix(dst, c, a as f32 / 255.0));
            }
        }
        pen += m.advance_width as i32;
    }
}

/// `a` の重みで `to` を `from` に混ぜる。
fn mix(from: Rgb, to: Rgb, a: f32) -> Rgb {
    let m = |f: u8, t: u8| (f as f32 + (t as f32 - f as f32) * a) as u8;
    (m(from.0, to.0), m(from.1, to.1), m(from.2, to.2))
}

/// 枠の色。白と黒が背景に溶けないよう、明るい色は暗く、
/// 暗い色は明るくずらした縁を描く。
fn border(c: Rgb) -> Rgb {
    let lum = 0.299 * c.0 as f32 + 0.587 * c.1 as f32 + 0.114 * c.2 as f32;
    let f = |v: u8| {
        if lum < 100.0 {
            (v as f32 + 90.0).min(255.0) as u8
        } else {
            (v as f32 * 0.55) as u8
        }
    };
    (f(c.0), f(c.1), f(c.2))
}

/// 塗りつぶした丸に、細い縁をつける。
/// 白と黒が背景に沈まないようにするため。
fn circle(buf: &mut [u32], cx: i32, cy: i32, r: i32, c: Rgb) {
    let fill = rgb(c);
    let edge = rgb(border(c));
    let rim = (r as f32 * 0.92) as i32;
    for y in (cy - r).max(0)..(cy + r).min(HEIGHT) {
        for x in (cx - r).max(0)..(cx + r).min(WIDTH) {
            let (dx, dy) = (x - cx, y - cy);
            let d2 = dx * dx + dy * dy;
            if d2 > r * r {
                continue;
            }
            buf[y as usize * W + x as usize] = if d2 > rim * rim { edge } else { fill };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **画面に出す字が、全部フォントに入っていること。** 入っていない字は
    /// 何も描かれず、落ちもしないので、目で見るまで気づけない。
    #[test]
    fn every_label_on_screen_is_in_the_font() {
        let f = font();
        let titles = crate::catalog(&crate::config::Config::default())
            .iter()
            .map(|g| g.title())
            .collect::<Vec<_>>();
        for s in crate::menu::LABELS.iter().chain(&titles) {
            for ch in s.chars() {
                assert_ne!(
                    f.lookup_glyph_index(ch),
                    0,
                    "「{s}」の「{ch}」がフォントに無い（fonts/README.md の手順で作り直すこと）"
                );
            }
        }
    }

    /// 窓がキャンバスと同じ形なら、そのまま縮める。
    #[test]
    fn a_window_of_the_same_shape_maps_straight() {
        assert_eq!(to_canvas((0.0, 0.0), (640, 360)), Some((0, 0)));
        assert_eq!(to_canvas((320.0, 180.0), (640, 360)), Some((640, 360)));
        assert_eq!(to_canvas((639.0, 359.0), (640, 360)), Some((1278, 718)));
    }

    /// **縦長の窓では上下に帯が入る。** 帯の上は画面の外。
    #[test]
    fn the_letterbox_is_outside_the_canvas() {
        // 1280×1080 の窓。上下に 180 ずつ帯が入る。
        assert_eq!(to_canvas((640.0, 100.0), (1280, 1080)), None);
        assert_eq!(to_canvas((640.0, 180.0), (1280, 1080)), Some((640, 0)));
        assert_eq!(to_canvas((640.0, 540.0), (1280, 1080)), Some((640, 360)));
        assert_eq!(to_canvas((640.0, 1000.0), (1280, 1080)), None);
    }

    #[test]
    fn a_collapsed_window_is_not_a_place() {
        assert_eq!(to_canvas((0.0, 0.0), (0, 0)), None);
    }
}
