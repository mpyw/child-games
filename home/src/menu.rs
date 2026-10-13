//! メニュー。遊びを並べて選ばせ、終わったら「もう1回」を訊く。
//!
//! ```text
//! メニュー ──決定──→ 遊ぶ ──終わる / 戻る──→ もう1回？
//!    ↑                 ↑                       │
//!    │                 └───────決定────────────┤
//!    └─────────────────────戻る────────────────┘
//! ```
//!
//! **装置にもファイルにも触らない。** 遊びと同じく `kit` のトレイト越しに
//! 借りるだけなので、進行は装置なしでテストできる（下のテスト）。
//! 実体は `io` にあり、`main` が繋ぐ。

use anyhow::Result;
use kit::{Devices, Ended, Game, Press, Rect, Scene, Shape, HEIGHT, WIDTH};

/// 「もう1回」のボタンの字。
const AGAIN: &str = "もう1回";

/// メニューが画面に出す字の全部（遊びの題は別）。**フォントのサブセットに
/// 入っていないと描かれない**ので、ウィンドウのテストがこれを見る。
#[cfg(test)]
pub const LABELS: &[&str] = &[AGAIN];

/// 「もう1回」の画面で遊びの絵をどれだけ暗くするか。真っ暗にすると
/// 終了に見える。
const DIM: f32 = 0.28;

/// 札の地の色。選んでいないとき。
const TILE: kit::Rgb = (44, 44, 44);
/// 選んでいる札の縁。離れたテレビからでも、どれが選ばれているか分かる色。
const FOCUS: kit::Rgb = (245, 245, 245);

pub struct Home {
    games: Vec<Box<dyn Game>>,
    dev: Devices,
    /// メニューで選んでいる札。**遊び終えて戻ってきたら、さっきの札を
    /// 選んだまま。** 同じ遊びをもう一度選ぶのが一番多いので。
    selected: usize,
    /// 1回遊んだら終わる（`--once`）。「もう1回」もメニューも出さない。
    once: bool,
}

/// 「もう1回」の画面でどう答えたか。
enum Next {
    Again,
    Menu,
    Closed,
}

impl Home {
    pub fn new(games: Vec<Box<dyn Game>>, dev: Devices, once: bool) -> Self {
        assert!(!games.is_empty(), "並べる遊びが無い");
        Self {
            games,
            dev,
            selected: 0,
            once,
        }
    }

    /// `start` が `Some` ならメニューを飛ばしてその遊びから始める
    /// （`--game`）。**入力が絶えるまで返らない。**
    pub fn run(&mut self, start: Option<usize>) -> Result<()> {
        let mut next = start;
        loop {
            let i = match next.take() {
                Some(i) => i,
                None => match self.choose() {
                    Some(i) => i,
                    None => return Ok(()),
                },
            };
            self.selected = i;
            loop {
                // **待っている間に押されたぶんは捨てる。** メニューや
                // 「もう1回」の画面で連打していたら、始めた瞬間に抜けてしまう。
                self.dev.control.discard();
                if self.games[i].play(&mut self.dev)? == Ended::Closed {
                    // 入力が絶えた。「もう1回」を訊く相手がいない。
                    return Ok(());
                }
                if self.once {
                    return Ok(());
                }
                match self.again(i) {
                    Next::Again => continue,
                    Next::Menu => break,
                    Next::Closed => return Ok(()),
                }
            }
        }
    }

    /// メニューで1つ選ばせる。入力が絶えたら `None`。
    fn choose(&mut self) -> Option<usize> {
        self.dev.control.discard();
        let n = self.games.len();
        let cols = columns(n);
        loop {
            self.dev.screen.show(self.menu());
            match self.dev.control.wait()? {
                Press::Ok => return Some(self.selected),
                Press::Pick(i) if i < n => return Some(i),
                Press::Point(x, y) => {
                    // 札の外を押しても何も起きない。テレビの前で狙わせる
                    // ものではないので、札は大きく取ってある。
                    if let Some(i) = tiles(n).iter().position(|t| t.contains(x, y)) {
                        return Some(i);
                    }
                }
                // **端で止める。回り込ませない。** 右を押し続けて最初に
                // 戻ると、どこに居るのか分からなくなる。
                Press::Left => self.selected = self.selected.saturating_sub(1),
                Press::Right => self.selected = (self.selected + 1).min(n - 1),
                Press::Up => self.selected = self.selected.saturating_sub(cols),
                Press::Down => {
                    if self.selected + cols < n {
                        self.selected += cols;
                    }
                }
                // メニューより手前は無い。
                Press::Back | Press::Pick(_) => {}
            }
        }
    }

    /// 遊び終えたあと「もう1回」を訊く。
    ///
    /// **決定はどこを押してもよい。** ボタンの絵は押せることを示すためで
    /// あって、当たり判定ではない。テレビの前で正確に狙わせるのは無理がある。
    fn again(&mut self, i: usize) -> Next {
        // 待っていることを画面に出してから待つ。黙って止まっていると、
        // 終わったのか固まったのか区別がつかない。
        let mut scene = self.games[i].backdrop();
        scene.shapes = scene.shapes.into_iter().map(|s| s.shaded(DIM)).collect();
        scene.shapes.push(Shape::Button {
            x: WIDTH / 2,
            y: HEIGHT / 2,
            px: HEIGHT / 10,
            text: AGAIN,
        });
        scene.caption = format!("── もう1回？（b でメニュー）── {}", self.games[i].title());
        self.dev.screen.show(scene);

        // 遊んでいる最中に押されたぶんは捨てる。子どもは歌っている間も
        // キーを叩くので、溜めたまま待ちに入るとフィナーレが鳴り終わった
        // 瞬間に再開してしまう。
        self.dev.control.discard();
        loop {
            match self.dev.control.wait() {
                Some(Press::Ok | Press::Point(..)) => return Next::Again,
                Some(Press::Back) => return Next::Menu,
                // 十字キーや番号では何も起きない。押し間違いで遊びを
                // 始めない。
                Some(_) => {}
                None => return Next::Closed,
            }
        }
    }

    /// メニューの場面。
    fn menu(&self) -> Scene {
        let mut shapes = Vec::new();
        let mut lines = vec!["── あそびをえらぶ ──".to_string()];
        for (i, (game, tile)) in self.games.iter().zip(tiles(self.games.len())).enumerate() {
            let chosen = i == self.selected;
            if chosen {
                let edge = tile.h / 40 + 4;
                shapes.push(Shape::RoundRect {
                    rect: tile.inset(-edge),
                    r: tile.h / 12 + edge,
                    fill: FOCUS,
                });
            }
            shapes.push(Shape::RoundRect {
                rect: tile,
                r: tile.h / 12,
                fill: TILE,
            });
            // 上の 3/4 に絵、下に題。
            let pad = tile.h / 14;
            let art = Rect {
                x: tile.x + pad,
                y: tile.y + pad,
                w: tile.w - 2 * pad,
                h: tile.h * 3 / 4 - pad,
            };
            shapes.extend(game.icon(art));
            let title = game.title();
            shapes.push(Shape::Text {
                x: tile.x + tile.w / 2,
                y: tile.y + tile.h * 7 / 8,
                px: title_px(title, tile),
                text: title,
                fill: FOCUS,
            });
            let mark = if chosen { "▶" } else { " " };
            lines.push(format!("  {mark} {}) {}", i + 1, title));
        }
        Scene {
            shapes,
            caption: lines.join("\n"),
        }
    }
}

/// 何列に並べるか。正方形に近くなるように。
fn columns(n: usize) -> usize {
    (1..=n).find(|c| c * c >= n).unwrap_or(1)
}

/// 札の置き場所。**大きく取る。** テレビの前で、リモコンでもマウスでも
/// 迷わず選べるように。1つしかなければ真ん中に1枚。
fn tiles(n: usize) -> Vec<Rect> {
    let cols = columns(n);
    let rows = n.div_ceil(cols);
    let (cw, ch) = (WIDTH / cols as i32, HEIGHT / rows as i32);
    // 札は 4:3 で、区画の 8 割まで。画面いっぱいだと札に見えない。
    let w = (cw * 8 / 10).min(ch * 8 / 10 * 4 / 3);
    let h = w * 3 / 4;
    (0..n)
        .map(|i| {
            let (c, r) = ((i % cols) as i32, (i / cols) as i32);
            Rect {
                x: c * cw + (cw - w) / 2,
                y: r * ch + (ch - h) / 2,
                w,
                h,
            }
        })
        .collect()
}

/// 題の字の大きさ。札の高さで決めて、幅に収まらなければ縮める。
///
/// 字の幅はフォントを持つ描き手にしか測れないので、**全角1字 ≒ 字の
/// 大きさ**として見積もる。題は仮名と記号なので外れない。
fn title_px(title: &str, tile: Rect) -> i32 {
    let chars = title.chars().count().max(1) as i32;
    (tile.h / 9).min(tile.w * 9 / 10 / chars)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use kit::{Control, Heard, Listener, Player, Screen, Sound, Timing};

    use super::*;

    /// 遊んだ記録。どの遊びを何回遊んだか。
    type Log = Rc<RefCell<Vec<&'static str>>>;

    /// 呼ばれたら記録して、決めておいた終わり方で返るだけの遊び。
    struct Fake {
        id: &'static str,
        log: Log,
        ends: Ended,
    }
    impl Game for Fake {
        fn id(&self) -> &'static str {
            self.id
        }
        fn title(&self) -> &'static str {
            self.id
        }
        fn sounds(&self) -> &'static [&'static str] {
            &[]
        }
        fn icon(&self, _area: Rect) -> Vec<Shape> {
            Vec::new()
        }
        fn backdrop(&self) -> Scene {
            Scene::default()
        }
        fn play(&mut self, _dev: &mut Devices) -> Result<Ended> {
            self.log.borrow_mut().push(self.id);
            Ok(self.ends)
        }
    }

    struct Mute;
    impl Player for Mute {
        fn begin(&self, _sound: Sound) -> Result<Timing> {
            unreachable!("メニューは鳴らさない")
        }
        fn silence(&self) {}
    }
    struct Deaf;
    impl Listener for Deaf {
        fn hear(&mut self, _vocabulary: &[&str]) -> Result<Heard> {
            unreachable!("メニューは聞かない")
        }
    }
    struct Blind;
    impl Screen for Blind {
        fn show(&mut self, _scene: Scene) {}
    }

    /// 台本どおりに押す。**待ちに入る前の掃除では消えない**ように、
    /// 掃除（`poll`）には何も見せず、`wait` にだけ順に渡す。尽きたら
    /// 入力が絶えたことにする。
    struct Presses(VecDeque<Press>);
    impl Control for Presses {
        fn poll(&mut self) -> Option<Press> {
            None
        }
        fn wait(&mut self) -> Option<Press> {
            self.0.pop_front()
        }
    }

    fn home(ids: &[&'static str], ends: Ended, presses: &[Press], once: bool) -> (Home, Log) {
        let log = Log::default();
        let games = ids
            .iter()
            .map(|&id| {
                Box::new(Fake {
                    id,
                    log: log.clone(),
                    ends,
                }) as Box<dyn Game>
            })
            .collect();
        let dev = Devices {
            player: Box::new(Mute),
            listener: Box::new(Deaf),
            screen: Box::new(Blind),
            control: Box::new(Presses(presses.iter().copied().collect())),
        };
        (Home::new(games, dev, once), log)
    }

    fn played(ids: &[&'static str], presses: &[Press]) -> Vec<&'static str> {
        let (mut h, log) = home(ids, Ended::Finished, presses, false);
        h.run(None).unwrap();
        let v = log.borrow().clone();
        v
    }

    /// 決定で今の札、もう1回で同じ遊び、戻るでメニュー。
    #[test]
    fn choose_play_again_and_back_to_the_menu() {
        use Press::*;
        assert_eq!(
            played(&["a", "b"], &[Ok, Ok, Back, Right, Ok]),
            ["a", "a", "b"],
        );
    }

    /// **端で止める。** 右を押し続けても最初の札に戻らない。
    #[test]
    fn the_selection_stops_at_the_edges() {
        use Press::*;
        assert_eq!(played(&["a", "b"], &[Right, Right, Right, Ok]), ["b"]);
        assert_eq!(played(&["a", "b"], &[Left, Left, Ok]), ["a"]);
    }

    /// 番号と、札を押したのでも選べる。範囲の外の番号では何も起きない。
    #[test]
    fn picking_by_number_or_by_touching_the_tile() {
        use Press::*;
        assert_eq!(played(&["a", "b"], &[Pick(9), Pick(1)]), ["b"]);
        let (x, y) = tiles(2)[1].center();
        assert_eq!(played(&["a", "b"], &[Point(x, y)]), ["b"]);
        // 札の外。
        assert_eq!(played(&["a", "b"], &[Point(0, 0), Ok]), ["a"]);
    }

    /// 「もう1回」はどこを押しても続ける。十字キーや番号では始めない。
    #[test]
    fn again_takes_any_click_but_not_the_arrows() {
        use Press::*;
        assert_eq!(
            played(&["a"], &[Ok, Right, Pick(0), Point(1, 1)]),
            ["a", "a"]
        );
    }

    /// **戻ってきたら、さっき選んだ札のまま。**
    #[test]
    fn the_menu_remembers_the_last_choice() {
        use Press::*;
        assert_eq!(played(&["a", "b"], &[Right, Ok, Back, Ok]), ["b", "b"]);
    }

    /// 入力が絶えたら、どこに居ても畳む。遊びの中で絶えたら「もう1回」も
    /// 訊かない。
    #[test]
    fn closed_input_ends_everything() {
        let (mut h, log) = home(&["a"], Ended::Closed, &[Press::Ok, Press::Ok], false);
        h.run(None).unwrap();
        assert_eq!(*log.borrow(), ["a"], "閉じたのに続けた");
    }

    /// `--game` で始めればメニューを飛ばす。`--once` なら1回で終わる。
    #[test]
    fn starting_directly_and_once() {
        let (mut h, log) = home(&["a", "b"], Ended::Finished, &[Press::Ok], true);
        h.run(Some(1)).unwrap();
        assert_eq!(*log.borrow(), ["b"]);
    }

    /// 札はどれも画面に収まり、重ならない。
    #[test]
    fn tiles_fit_on_the_screen_without_overlapping() {
        for n in 1..=9 {
            let ts = tiles(n);
            assert_eq!(ts.len(), n);
            for (i, a) in ts.iter().enumerate() {
                assert!(a.x >= 0 && a.y >= 0, "{n}枚: {a:?}");
                assert!(a.x + a.w <= WIDTH && a.y + a.h <= HEIGHT, "{n}枚: {a:?}");
                for b in &ts[i + 1..] {
                    let apart = a.x + a.w <= b.x
                        || b.x + b.w <= a.x
                        || a.y + a.h <= b.y
                        || b.y + b.h <= a.y;
                    assert!(apart, "{n}枚: {a:?} と {b:?} が重なる");
                }
            }
        }
    }

    /// 題は札の幅に収まる。
    #[test]
    fn titles_fit_in_their_tile() {
        let t = tiles(1)[0];
        let title = "どんないろがすき？";
        let px = title_px(title, t);
        assert!(
            px * title.chars().count() as i32 <= t.w,
            "{px}px ははみ出す"
        );
    }
}
