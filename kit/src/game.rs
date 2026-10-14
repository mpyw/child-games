//! 遊びの口。メニューに並び、選ばれたら装置を借りて遊ぶ。

use anyhow::Result;

use crate::{Camera, Control, Listener, Player, Rect, Scene, Screen, Shape};

/// 遊びに貸す装置。**持ち主は `home`。** 遊びが終われば返ってくるので、
/// 次に選ばれた遊びが同じものを使う。
///
/// 開き直さないのは、マイクと whisper の初期化に秒単位かかるため。
pub struct Devices {
    pub player: Box<dyn Player>,
    pub listener: Box<dyn Listener>,
    pub screen: Box<dyn Screen>,
    pub control: Box<dyn Control>,
    /// 使わない遊びもある。使った遊びは、終わるときに `rest` で休ませる。
    pub camera: Box<dyn Camera>,
}

/// ひと続き遊び終えたあと、どうなったか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// 遊び終えた。**途中で「戻る」を押されたのもここ。** どちらも
    /// 次は「もう1回」を訊く。親が「この回はここまで」と言っただけで、
    /// 続きを訊く相手はまだ居る。
    Finished,
    /// 入力そのものが絶えた。「もう1回」を訊く相手もいないので畳む。
    Closed,
}

pub trait Game {
    /// `--game` で指す名前。素材の在処（`games/<id>/assets/`）もこれで決まる。
    fn id(&self) -> &'static str;

    /// メニューに出す題。
    ///
    /// **画面に出す字はフォントのサブセットに入っていないと描かれない。**
    /// 題を変えたらフォントを作り直すこと（`home/fonts/README.md`）。
    /// 足りなければ `home` のテストが落ちる。
    fn title(&self) -> &'static str;

    /// 題のほかに画面に出す字の全部。点数の数字など。
    ///
    /// 題と同じく、フォントのサブセットに入っていないと描かれない。
    /// `home` のテストがこれを見る。図形だけで描く遊びは空のままでよい。
    fn glyphs(&self) -> &'static str {
        ""
    }

    /// 鳴らしうる素材の名前の全部。`home` が起動時に揃っているかを確かめ、
    /// 埋め込みビルドではこれを表と突き合わせる。
    fn sounds(&self) -> &'static [&'static str];

    /// メニューの札に描く絵。`area` の中に収めること。
    fn icon(&self, area: Rect) -> Vec<Shape>;

    /// 「もう1回」の画面で後ろに敷く絵。
    ///
    /// 暗くしてボタンを載せるのは `home` の側。遊びが終わったのではなく
    /// 続きがある、と見えるように、真っ暗にはせず遊びの絵を残す。
    fn backdrop(&self) -> Scene;

    /// ひと続き遊ぶ。「もう1回」を訊くのは `home` の仕事なので、ここでは
    /// 訊かずに返る。
    fn play(&mut self, dev: &mut Devices) -> Result<Ended>;
}
