//! 何をどこに映すか。
//!
//! 遊びは**図形を並べた場面**（`Scene`）を組んで渡すだけで、画素は知らない。
//! 実際に描くのは `home::io::screen` で、本命はウィンドウ、ターミナルは
//! 動作確認用（`caption` だけを出す）。
//!
//! 以前は `Frame::Palette(Color)` のように色当て専用の場面を描き手が直に
//! 知っていた。遊びが1つなら困らないが、2つ目を足すたびに描き手を
//! 書き換えることになる。図形まで降ろしておけば、描き手は遊びを知らずに済む。

/// 光の三原色。
pub type Rgb = (u8, u8, u8);

/// キャンバスの大きさ。**遊びはこの座標で描く。** 実際の画面に合わせて
/// 伸ばすのは描き手の仕事（縦横比は保つ）。
pub const WIDTH: i32 = 1280;
pub const HEIGHT: i32 = 720;

/// 何も描いていないところの色。真っ黒にしないのは、黒い丸が沈むため。
pub const BACKGROUND: Rgb = (16, 16, 16);

/// 長方形。キャンバスの座標で、`(x, y)` が左上。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    /// キャンバス全体。
    pub const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        w: WIDTH,
        h: HEIGHT,
    };

    pub const fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    /// 端は左上を含み、右下を含まない。隣り合う札が両方当たらないように。
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        self.x <= x && x < self.x + self.w && self.y <= y && y < self.y + self.h
    }

    /// 四辺を `d` ずつ内側へ寄せる。
    pub const fn inset(&self, d: i32) -> Rect {
        Rect {
            x: self.x + d,
            y: self.y + d,
            w: self.w - 2 * d,
            h: self.h - 2 * d,
        }
    }
}

/// 描くもの。**並べた順に重ねる**（後のものが上）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// 塗った丸。
    ///
    /// **縁は描き手が付ける。** 白と黒が背景に沈まないよう、明るい色は
    /// 暗く、暗い色は明るくずらした細い縁が必ず付く。
    Circle { x: i32, y: i32, r: i32, fill: Rgb },
    /// 角の丸い長方形。
    RoundRect { rect: Rect, r: i32, fill: Rgb },
    /// 光の当たった球。左上から光が当たり、右下が暗くなる。てかりも付く。
    ///
    /// **単色の丸では球に見えない。** たこ焼きのように転がすものは、明るさが
    /// 面の向きで変わらないと、平たい円盤が回っているように見える。
    ///
    /// `cap` があれば、球の一部をその色で塗る（焼けた面など）。
    Sphere {
        x: i32,
        y: i32,
        r: i32,
        fill: Rgb,
        cap: Option<Cap>,
    },
    /// 塗った三角形。3つの頂点はどちら回りでもよい。縁は付かない。
    ///
    /// 魚の尾やサメの背びれのような、尖ったものを描くため。丸を積んで
    /// 作ると、でこぼこの塊にしか見えない。
    Triangle { points: [(i32, i32); 3], fill: Rgb },
    /// 文字。`(x, y)` が字面の中心、`px` が字の大きさ。
    Text {
        x: i32,
        y: i32,
        px: i32,
        text: &'static str,
        fill: Rgb,
    },
    /// 押せることを示すボタン。`(x, y)` が中心、`px` が字の大きさ。
    ///
    /// **大きさは字から決まるので描き手に任せる。** 字の幅はフォントを
    /// 持っている側にしか測れない。
    Button {
        x: i32,
        y: i32,
        px: i32,
        text: &'static str,
    },
}

/// 球の一部を別の色で塗る。
///
/// `toward` の向き（画面の右が x、下が y、手前が z）を中心に、面の向きとの
/// 内積が `1 - size` より大きい所を塗る。`size` が 0 なら塗らず、1 なら半球、
/// 2 なら全部。`toward` を回せば、塗った面が球の上を転がる。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cap {
    pub fill: Rgb,
    pub toward: (f32, f32, f32),
    pub size: f32,
}

impl Shape {
    /// 色を `f` 倍する。「もう1回」の画面で遊びの絵を暗く敷くため。
    pub fn shaded(self, f: f32) -> Shape {
        let g = |c: Rgb| {
            let m = |v: u8| (v as f32 * f).clamp(0.0, 255.0) as u8;
            (m(c.0), m(c.1), m(c.2))
        };
        match self {
            Shape::Circle { x, y, r, fill } => Shape::Circle {
                x,
                y,
                r,
                fill: g(fill),
            },
            Shape::RoundRect { rect, r, fill } => Shape::RoundRect {
                rect,
                r,
                fill: g(fill),
            },
            Shape::Triangle { points, fill } => Shape::Triangle {
                points,
                fill: g(fill),
            },
            Shape::Sphere { x, y, r, fill, cap } => Shape::Sphere {
                x,
                y,
                r,
                fill: g(fill),
                cap: cap.map(|c| Cap {
                    fill: g(c.fill),
                    ..c
                }),
            },
            Shape::Text {
                x,
                y,
                px,
                text,
                fill,
            } => Shape::Text {
                x,
                y,
                px,
                text,
                fill: g(fill),
            },
            // ボタンは暗くしない。暗いボタンは押せるように見えない。
            b @ Shape::Button { .. } => b,
        }
    }
}

/// 画素を並べた絵。左上から1行ずつ。
///
/// **`Shape` には入れない。** `Shape` は `Copy` で、画素の束を持てない。
/// 1枚の絵は場面に1つあれば足りる（カメラの鏡像）ので、`Scene` の側に
/// 欄を設ける。画素は `Arc` で持つので、場面を複製しても写さない。
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    /// `width * height` 個。
    pub pixels: std::sync::Arc<[Rgb]>,
}

impl Image {
    /// 一色で塗った絵。
    pub fn filled(width: usize, height: usize, c: Rgb) -> Image {
        Image {
            width,
            height,
            pixels: vec![c; width * height].into(),
        }
    }

    /// `(x, y)` の画素。
    pub fn at(&self, x: usize, y: usize) -> Rgb {
        self.pixels[y * self.width + x]
    }

    /// 色を `f` 倍する。`Shape::shaded` と同じ。
    pub fn shaded(&self, f: f32) -> Image {
        let m = |v: u8| (v as f32 * f).clamp(0.0, 255.0) as u8;
        Image {
            width: self.width,
            height: self.height,
            pixels: self
                .pixels
                .iter()
                .map(|c| (m(c.0), m(c.1), m(c.2)))
                .collect(),
        }
    }
}

/// 今なにを映すか。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    /// 図形の下に敷く絵。**キャンバス全体に伸ばす**（縦横比は描き手が
    /// 保つとは限らないので、16:9 で渡すこと）。無ければ `BACKGROUND`。
    pub image: Option<Image>,
    /// ウィンドウに描くもの。
    pub shapes: Vec<Shape>,
    /// ターミナルに出す1行。**動作確認用なので進行が追えれば足りる。**
    /// 空なら何も出さない。
    pub caption: String,
}

impl Scene {
    /// 図形も絵も `f` 倍に暗くする。「もう1回」の画面で後ろに敷くため。
    /// ボタンは暗くしない（`Shape::shaded`）。
    pub fn shaded(self, f: f32) -> Scene {
        Scene {
            image: self.image.map(|i| i.shaded(f)),
            shapes: self.shapes.into_iter().map(|s| s.shaded(f)).collect(),
            caption: self.caption,
        }
    }
}

pub trait Screen: Send {
    fn show(&mut self, scene: Scene);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbours_do_not_both_contain_the_border() {
        let a = Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        };
        let b = Rect { x: 10, ..a };
        assert!(a.contains(9, 5) && !a.contains(10, 5));
        assert!(b.contains(10, 5));
    }

    #[test]
    fn shading_leaves_the_button_alone() {
        let c = Shape::Circle {
            x: 0,
            y: 0,
            r: 1,
            fill: (200, 100, 0),
        };
        assert_eq!(
            c.shaded(0.5),
            Shape::Circle {
                x: 0,
                y: 0,
                r: 1,
                fill: (100, 50, 0)
            }
        );
        let b = Shape::Button {
            x: 0,
            y: 0,
            px: 10,
            text: "x",
        };
        assert_eq!(b.shaded(0.1), b);
    }
}
