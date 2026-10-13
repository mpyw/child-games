//! 進行。
//!
//! ```text
//! intro
//!   ↓
//! ┌→ question            どんないろがすき？（ト長調）
//! │    ↓ 最大5秒待つ（何も言わなければランダムな色）
//! │  <color>              その色の節（5小節・ト長調）
//! │    ↓
//! │  tail / tail-lead     節の最終小節。間奏へ向かうときだけ助走つき
//! │    ↓
//! │  bridge / interlude   3周に1回、交互に挟む
//! └──┘                    「ぜんぶ！」と言うまで無限ループ
//!      ↓「ぜんぶ！」
//!    finale               転調 → ぜんぶの節 → エンディング
//! ```
//!
//! ここで返る。「もう1回」を訊くのは `home` の仕事。
//!
//! 要点は**無反応にしないこと**。判定できなくても必ず何かを鳴らす。
//!
//! ただし**フィナーレのあとだけはポリシーが反転する**。あそこは遊びの
//! ループの外なので、無反応は「もう終わり」と読んでよい。ここで
//! ランダムに倒すと永久に終われなくなる。

use std::time::{Duration, Instant};

use anyhow::Result;
use kit::{Devices, Ended, Heard, Rect, Scene, Shape};
use strum::{EnumCount, VariantArray};

use crate::color::{Answer, Color};
use crate::config::Config;
use crate::cue::Cue;
use crate::look;
use crate::matcher::Matcher;

/// 「やめる」を見に行く間隔。**歌の途中で抜けるので細かく見る。**
/// 30fps の描画より粗い刻みだと、押してから止まるまでが目に見える。
const POLL: Duration = Duration::from_millis(50);

pub struct DonnaIro {
    matcher: Matcher,
    /// whisper に教える語。`Answer` から作るので、色を足せばここも伸びる。
    vocabulary: Vec<&'static str>,
    insert_every: u32,
    flash: Duration,
}

impl DonnaIro {
    pub fn new(cfg: &Config) -> Self {
        Self {
            matcher: Matcher::new(cfg.head_segments),
            vocabulary: Answer::every().iter().map(|a| a.reading()).collect(),
            insert_every: cfg.insert_every,
            flash: cfg.flash(),
        }
    }

    /// 鳴らして、鳴り終わるまで待つ。
    ///
    /// **待っている間に「やめる」を見る。** 来たら鳴っている音を止めて
    /// すぐ返る。止めないと画面だけ切り替わって歌が流れ続ける。
    ///
    /// `quiet` が真なら、末尾の無音を待たずに音が鳴り止んだ時点で返る。
    /// `question.wav` の合いの手枠がそこに入っていて、応答の窓になる。
    ///
    /// 戻り値は**この回を続けてよいか**。
    fn sound(dev: &mut Devices, cue: Cue, quiet: bool) -> Result<bool> {
        let timing = dev.player.begin(cue.sound())?;
        let end = Instant::now() + if quiet { timing.audible } else { timing.total };
        loop {
            if dev.control.stop_requested() {
                dev.player.silence();
                return Ok(false);
            }
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(true);
            }
            std::thread::sleep(POLL.min(left));
        }
    }

    /// イントロから「ぜんぶ！」のフィナーレまで、ひと続き。
    ///
    /// 周回数はここで閉じているので、もう1回のたびに区切りの周期も
    /// 頭から数え直す。前回の続きから間奏が来ると唐突になる。
    ///
    /// **途中で「戻る」を押されても `Finished`。** 行き先は最後まで遊んだ
    /// ときと同じ「もう1回」の画面になる。
    fn play_through(&mut self, dev: &mut Devices) -> Result<Ended> {
        // 待っている間に押されたぶんは `home` が捨ててから渡してくる。
        dev.screen.show(look::palette(Color::VARIANTS));
        if !Self::sound(dev, Cue::Intro, false)? {
            return Ok(Ended::Finished);
        }

        // 「ぜんぶ！」と言うまで無限に続く。何度でも好きな色を
        // 答えられるのがこの遊びの本体なので、回数の上限は設けない。
        let mut round: u32 = 0;
        loop {
            round += 1;

            // まだ色が決まっていないので全色を出す。
            dev.screen.show(look::palette(Color::VARIANTS));

            // 質問は**鳴り止んだ時点で返る**。末尾の合いの手枠は
            // 無音のまま裏で流れ続け、そこが応答の窓になる。
            if !Self::sound(dev, Cue::Question, true)? {
                return Ok(Ended::Finished);
            }

            let answer = match dev.listener.hear(&self.vocabulary)? {
                Heard::Said(text) => {
                    // **判定を出すのはここだけ。** `Matcher` の中で書くと、
                    // 変異を掛けるテストが数千行のログを吐く。落選まで
                    // 出しておかないと、外したときに手掛かりが残らない。
                    //
                    // stderr へ直に書く。装置でもファイルでもないので、
                    // 遊びの縛り（`lib.rs` を見ること）には触れない。
                    let verdict = self.matcher.find(&text);
                    eprintln!("  {verdict}");
                    // **`None` は「測ってすらいない」だけ。** 遠かったものは
                    // 判定の側で一番近いものへ倒してある。足切りして黙るより、
                    // それっぽいものを鳴らすほうが当たる（`matcher.rs`）。
                    verdict.answer()
                }
                Heard::Nothing => None,
                Heard::Closed => return Ok(Ended::Closed),
            };

            // **聞き取りの間に押されたぶんもここで拾う。** 録音は止められ
            // ないので、抜けるのは窓が閉じたあと（最大 listen.max_seconds
            // ぶん遅れる）。歌の途中は上の `sound` が即座に抜ける。
            if dev.control.stop_requested() {
                return Ok(Ended::Finished);
            }

            if answer == Some(Answer::All) {
                self.finale(dev)?;
                return Ok(Ended::Finished);
            }

            // 何も聞こえなかったときだけランダムな色。黙ってはいけない。
            // **声が届いていれば、遠くてもここには来ない**（判定が一番近い
            // ものへ倒す）。ここに来るのは無言と、1音だけのときだけ。
            //
            // ここで All に倒してはならない。何も言っていないのに終わる。
            let color = match answer {
                Some(Answer::Single(c)) => c,
                _ => Color::random(),
            };
            dev.screen.show(look::single(color));
            if !Self::sound(dev, Cue::Color(color), false)? {
                return Ok(Ended::Finished);
            }

            // 3周に1回、区切りを挟む。同じ質問と節の往復だけだと単調になる。
            // 挟むものはブリッジと間奏を交互に入れ替える。同じ区切りが
            // 毎回続くとそれ自体が単調になるため。
            let insert = round.is_multiple_of(self.insert_every);
            let interlude_next = insert && (round / self.insert_every).is_multiple_of(2);

            // 節の最終小節。間奏を launch する助走はアウフタクトで
            // この小節に属するので、間奏へ向かうときだけ差し替える。
            let tail = if interlude_next {
                Cue::TailLead
            } else {
                Cue::Tail
            };
            if !Self::sound(dev, tail, false)? {
                return Ok(Ended::Finished);
            }

            if insert {
                dev.screen.show(look::palette(Color::VARIANTS));
                let between = if interlude_next {
                    Cue::Interlude
                } else {
                    Cue::Bridge
                };
                if !Self::sound(dev, between, false)? {
                    return Ok(Ended::Finished);
                }
            }
        }
    }

    /// 転調 → ぜんぶの節 → エンディング。
    ///
    /// 鳴らし終わるのを待つのではなく、全長を受け取って鳴っている間に
    /// 色を差し替える。「ぜんぶ」と答えたのだから、ぜんぶの色が
    /// 次々に出るほうが締めくくりらしい。
    fn finale(&mut self, dev: &mut Devices) -> Result<()> {
        let timing = dev.player.begin(Cue::Finale.sound())?;
        let end = Instant::now() + timing.total;
        let mut order = Color::ALL;
        while Instant::now() < end {
            // **フィナーレも途中で抜けられる。** 行き先は「もう1回」の
            // 画面で、最後まで鳴らした場合と同じなので、返り値は要らない。
            if dev.control.stop_requested() {
                dev.player.silence();
                break;
            }
            order = shuffle(&order);
            dev.screen.show(look::palette(&order));
            let left = end.saturating_duration_since(Instant::now());
            std::thread::sleep(self.flash.min(left));
        }
        dev.screen.show(look::palette(Color::VARIANTS));
        Ok(())
    }
}

impl kit::Game for DonnaIro {
    fn id(&self) -> &'static str {
        crate::ID
    }

    fn title(&self) -> &'static str {
        "どんないろがすき？"
    }

    fn sounds(&self) -> &'static [&'static str] {
        &crate::SOUNDS
    }

    /// 全色を縮めて並べる。遊んでいるときの絵そのままなので、札を見れば
    /// どの遊びか分かる。
    fn icon(&self, area: Rect) -> Vec<Shape> {
        look::grid(area, Color::VARIANTS)
    }

    fn backdrop(&self) -> Scene {
        look::palette(Color::VARIANTS)
    }

    fn play(&mut self, dev: &mut Devices) -> Result<Ended> {
        self.play_through(dev)
    }
}

/// どの位置も前回と違う色になるように並べ替える。
///
/// 同じ場所が同じ色のままだと「入れ替わった」ように見えない。
/// 完全順列（derangement）になるまで引き直す。12色なら
/// 当たる確率が 1/e ≒ 37% なので、数回で決まる。
fn shuffle(prev: &[Color; Color::COUNT]) -> [Color; Color::COUNT] {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    loop {
        let mut next = *prev;
        next.shuffle(&mut rng);
        if next.iter().zip(prev.iter()).all(|(a, b)| a != b) {
            return next;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use kit::{Control, Game, Listener, Player, Press, Screen, Sound, Timing};
    use strum::VariantArray;

    use super::*;

    /// 鳴らした順を記録するだけ。長さ0なので待ち時間が消える。
    #[derive(Clone, Default)]
    struct Tape {
        cues: Rc<RefCell<Vec<Cue>>>,
        /// 止めた回数。**画面だけ切り替えて鳴らし続ける事故を捕まえる。**
        silenced: Rc<RefCell<usize>>,
    }

    impl Player for Tape {
        fn begin(&self, sound: Sound) -> Result<Timing> {
            // 名前から引き直す。並びを `Cue` のまま検査したいので。
            assert_eq!(sound.game, crate::ID, "よその遊びの素材を鳴らした");
            let cue = Cue::every()
                .into_iter()
                .find(|c| c.stem() == sound.name)
                .expect("知らない素材を鳴らした");
            self.cues.borrow_mut().push(cue);
            Ok(Timing {
                total: Duration::ZERO,
                audible: Duration::ZERO,
            })
        }
        fn silence(&self) {
            *self.silenced.borrow_mut() += 1;
        }
    }

    /// 台本どおりに答える。`None` は「聞き取れなかった」。
    struct Script(std::vec::IntoIter<Option<&'static str>>);

    impl Listener for Script {
        fn hear(&mut self, vocabulary: &[&str]) -> Result<Heard> {
            // 語彙が渡っていないと、whisper が漢字に変換してしまう。
            assert!(vocabulary.contains(&"ぜんぶ"), "語彙が渡っていない");
            Ok(match self.0.next() {
                Some(Some(answer)) => Heard::Said(answer.to_string()),
                Some(None) => Heard::Nothing,
                // 台本が尽きたら「ぜんぶ」。放っておくとランダムな色で
                // 回り続けてテストが返らない。
                None => Heard::Said("ぜんぶ".to_string()),
            })
        }
    }

    struct Blind;
    impl Screen for Blind {
        fn show(&mut self, _scene: Scene) {}
    }

    /// 何も押されない。
    struct Idle;
    impl Control for Idle {
        fn poll(&mut self) -> Option<Press> {
            None
        }
        fn wait(&mut self) -> Option<Press> {
            None
        }
    }

    /// `after` 回目に覗かれたところで「戻る」が押されている。
    struct BackAfter(usize);
    impl Control for BackAfter {
        fn poll(&mut self) -> Option<Press> {
            if self.0 == 0 {
                return None;
            }
            self.0 -= 1;
            // 何も無い回は `None` で返り、`stop_requested` の掃除を抜ける。
            (self.0 == 0).then_some(Press::Back)
        }
        fn wait(&mut self) -> Option<Press> {
            None
        }
    }

    fn devices(
        tape: &Tape,
        listener: impl Listener + 'static,
        control: impl Control + 'static,
    ) -> Devices {
        Devices {
            player: Box::new(tape.clone()),
            listener: Box::new(listener),
            screen: Box::new(Blind),
            control: Box::new(control),
        }
    }

    /// 台本を渡して1回遊ばせ、鳴った順を返す。
    fn played(answers: Vec<Option<&'static str>>) -> Vec<Cue> {
        let tape = Tape::default();
        let mut dev = devices(&tape, Script(answers.into_iter()), Idle);
        let ended = DonnaIro::new(&Config::default()).play(&mut dev).unwrap();
        assert_eq!(ended, Ended::Finished);
        let cues = tape.cues.borrow().clone();
        cues
    }

    /// **「戻る」で歌の途中から抜ける。** 終わりではないので `Finished` で
    /// 返り、`home` が「もう1回」を訊く。
    #[test]
    fn stopping_midway_returns_without_finishing_the_song() {
        let tape = Tape::default();
        // 2回目に覗かれたところ（質問を鳴らしている最中）でやめる。
        let mut dev = devices(&tape, Script(vec![].into_iter()), BackAfter(2));
        let ended = DonnaIro::new(&Config::default()).play(&mut dev).unwrap();

        assert_eq!(ended, Ended::Finished, "やめただけで畳んではいけない");
        assert_eq!(*tape.cues.borrow(), [Cue::Intro, Cue::Question]);
        // **鳴っている音を止めていること。** 止めないと画面だけ切り替わって
        // 歌が流れ続ける。
        assert_eq!(*tape.silenced.borrow(), 1, "止めていない");
    }

    /// 遊びの口から見える素材の一覧が、実際に鳴らすものと揃っていること。
    /// `home` はこれで起動時の検査をする。
    #[test]
    fn declared_sounds_are_every_cue() {
        let game = DonnaIro::new(&Config::default());
        assert_eq!(game.id(), crate::ID);
        let mut declared: Vec<&str> = game.sounds().to_vec();
        let mut every: Vec<&str> = Cue::every().iter().map(|c| c.stem()).collect();
        declared.sort_unstable();
        every.sort_unstable();
        assert_eq!(declared, every);
    }

    #[test]
    fn answering_a_color_plays_its_phrase() {
        assert_eq!(
            played(vec![Some("あか")]),
            [
                Cue::Intro,
                Cue::Question,
                Cue::Color(Color::Red),
                Cue::Tail,
                // 台本切れ →「ぜんぶ」
                Cue::Question,
                Cue::Finale,
            ]
        );
    }

    /// 入力そのものが閉じたら、そこで畳む。
    ///
    /// **`--keyboard` をパイプで流し込むと EOF がここに来る。** 「無言」と
    /// 同じ扱いにしていた頃は、ランダムな色を上限まで鳴らし続けて返らなかった。
    /// フィナーレを通らずに `Closed` で返るのが正しい。
    #[test]
    fn closed_input_ends_the_game_without_a_finale() {
        struct Eof;
        impl Listener for Eof {
            fn hear(&mut self, _vocabulary: &[&str]) -> Result<Heard> {
                Ok(Heard::Closed)
            }
        }
        let tape = Tape::default();
        let mut dev = devices(&tape, Eof, Idle);
        let ended = DonnaIro::new(&Config::default()).play(&mut dev).unwrap();

        assert_eq!(ended, Ended::Closed);
        let cues = tape.cues.borrow().clone();
        assert_eq!(
            cues,
            [Cue::Intro, Cue::Question],
            "余計に鳴っている: {cues:?}"
        );
    }

    #[test]
    fn unheard_answer_still_plays_some_color() {
        let cues = played(vec![None]);
        // 黙ってはいけない。ランダムな色に倒す。
        assert!(
            matches!(cues[2], Cue::Color(_)),
            "聞き取れなかったのに色が鳴っていない: {cues:?}"
        );
        // 事故で終わってはいけない。
        assert_ne!(cues[2], Cue::Finale);
    }

    /// 遠い声でも、**判定が倒した色がそのまま鳴ること。**
    ///
    /// 足切りしていた頃はここがランダムだったので、繋ぎ間違えても気づけない。
    /// 「あたん」は実機のログで、子どもは「あか」と言っていた（`matcher.rs`）。
    #[test]
    fn a_distant_answer_plays_the_closest_color() {
        let cues = played(vec![Some("あたん")]);
        assert_eq!(cues[2], Cue::Color(Color::Red), "{cues:?}");
    }

    #[test]
    fn a_break_comes_every_third_round() {
        let cues = played(vec![Some("あか"); 6]);
        let breaks: Vec<Cue> = cues
            .iter()
            .filter(|c| matches!(c, Cue::Bridge | Cue::Interlude))
            .copied()
            .collect();
        // 6周で2回。ブリッジと間奏が交互に入る。
        assert_eq!(breaks, [Cue::Bridge, Cue::Interlude], "{cues:?}");
    }

    #[test]
    fn only_the_interlude_gets_a_run_up() {
        let cues = played(vec![Some("あか"); 6]);
        // 助走（tail-lead）は間奏の直前だけ。ブリッジの前は素の tail。
        let lead = cues
            .iter()
            .position(|c| *c == Cue::TailLead)
            .expect("助走が無い");
        assert_eq!(cues[lead + 1], Cue::Interlude, "{cues:?}");
        assert_eq!(cues.iter().filter(|c| **c == Cue::TailLead).count(), 1);
    }

    /// **区切りの周期は回ごとに数え直す。** 前の回の続きから間奏が来ると
    /// 唐突になる。同じ遊びを2回遊ばせて、2回目も頭から数えていること。
    #[test]
    fn each_play_counts_rounds_from_the_start() {
        let tape = Tape::default();
        let mut game = DonnaIro::new(&Config::default());
        for _ in 0..2 {
            let mut dev = devices(&tape, Script(vec![Some("あか"); 2].into_iter()), Idle);
            game.play(&mut dev).unwrap();
        }
        let cues = tape.cues.borrow().clone();
        // 2周で台本が切れるので、どちらの回も3周目（区切り）に届かない。
        assert!(
            !cues
                .iter()
                .any(|c| matches!(c, Cue::Bridge | Cue::Interlude)),
            "前の回の周回を引き継いでいる: {cues:?}"
        );
        assert_eq!(cues.iter().filter(|c| **c == Cue::Intro).count(), 2);
    }

    #[test]
    fn shuffle_moves_every_position() {
        let mut order = Color::ALL;
        for _ in 0..200 {
            let next = shuffle(&order);
            // 全色が1つずつ残っている
            let mut a = next;
            a.sort_by_key(|c| c.stem());
            let mut b = Color::VARIANTS.to_vec();
            b.sort_by_key(|c| c.stem());
            assert_eq!(a.as_slice(), b, "色が増減している");
            // どの位置も色が変わっている
            assert!(
                next.iter().zip(order.iter()).all(|(x, y)| x != y),
                "同じ位置に同じ色が残った"
            );
            order = next;
        }
    }
}
