//! 進行。
//!
//! ```text
//! song 開始 ─→ ┌→ カメラの絵を1枚待つ（毎秒30枚くらい）
//!              │   動いた所を出す → 重なった風船を割る（効果音）
//!              │   風船を足す・浮かせる・抜けたら消す → 鏡と風船を映す
//!              └── song が鳴り終わるまで（「戻る」なら途中で抜ける）
//!          ─→ fanfare（割った風船を並べる）
//! ```
//!
//! ここで返る。「もう1回」を訊くのは `home` の仕事。
//!
//! **時間切れという失敗は無い。** 1回の長さは曲の長さで決まり、終わりは
//! 必ずお祝いになる。2歳児は制限時間を理解できない。

use std::time::{Duration, Instant};

use anyhow::Result;
use kit::{Devices, Ended, Press, Rect, Rgb, Scene, Shape};

use crate::config::Config;
use crate::cue::Cue;
use crate::field::Field;
use crate::look;
use crate::motion::Motion;

/// お祝いの間に画面を描き直す間隔。カメラを見ないので自分で刻む。
const TICK: Duration = Duration::from_millis(33);

pub struct BalloonCrash {
    cfg: Config,
    /// 前の回に割った風船。「もう1回」の画面で後ろに敷く。
    popped: Vec<Rgb>,
}

impl BalloonCrash {
    pub fn new(cfg: &Config) -> Self {
        Self {
            cfg: cfg.clone(),
            popped: Vec::new(),
        }
    }

    /// 曲が終わるまで遊んで、お祝いまで。**カメラを休ませるのは呼び出し側**
    /// （どこで抜けても休ませるため）。
    fn round(&mut self, dev: &mut Devices, seed: u64) -> Result<Ended> {
        self.popped.clear();
        let mut field = Field::new(&self.cfg, seed);
        let mut motion = Motion::default();
        let mut pops = 0usize;

        let song = dev.player.begin(Cue::Song.sound())?;
        let start = Instant::now();
        let mut last = Duration::ZERO;
        loop {
            // 絵を待つ。1周の速さはここで決まる。
            let mirror = dev.camera.frame()?;
            let now = start.elapsed();
            if now >= song.total {
                break;
            }

            let mut popped = 0;
            while let Some(p) = dev.control.poll() {
                match p {
                    Press::Back => {
                        dev.player.silence();
                        self.popped = field.popped().to_vec();
                        return Ok(Ended::Finished);
                    }
                    // マウスで押しても割れる。カメラが無いときに試す口。
                    Press::Point(x, y) => popped += field.poke(now, x, y) as usize,
                    // 十字キーと決定では何も起きない。押し間違いで遊びを切らない。
                    _ => {}
                }
            }

            let moved = motion.see(&mirror, self.cfg.threshold);
            popped += field.touch(now, &moved);
            // **1周に1回だけ鳴らす。** 同時に2つ割れても、効果音の口は1本
            // なので重ねられない。
            if popped > 0 {
                dev.player.effect(Cue::Pop(pops).sound())?;
                pops += popped;
            }

            field.step(now, now.saturating_sub(last).as_secs_f32());
            last = now;
            dev.screen.show(look::playing(&mirror, &field, &moved, now));
        }

        self.popped = field.popped().to_vec();
        dev.camera.rest();
        let fanfare = dev.player.begin(Cue::Fanfare.sound())?;
        let start = Instant::now();
        while start.elapsed() < fanfare.total {
            if dev.control.stop_requested() {
                dev.player.silence();
                return Ok(Ended::Finished);
            }
            dev.screen.show(look::finale(&self.popped, start.elapsed()));
            std::thread::sleep(TICK);
        }
        Ok(Ended::Finished)
    }
}

impl kit::Game for BalloonCrash {
    fn id(&self) -> &'static str {
        crate::ID
    }

    fn title(&self) -> &'static str {
        "ふうせん パン！"
    }

    fn sounds(&self) -> &'static [&'static str] {
        &crate::SOUNDS
    }

    fn icon(&self, area: Rect) -> Vec<Shape> {
        look::icon(area)
    }

    fn backdrop(&self) -> Scene {
        look::finale(&self.popped, Duration::ZERO)
    }

    fn play(&mut self, dev: &mut Devices) -> Result<Ended> {
        let result = self.round(dev, rand::random());
        // **どこで抜けてもカメラを休ませる。** ランプが点いたままだと、
        // 次に選ぶどんないろがすき？の間も撮られているように見える。
        dev.camera.rest();
        result
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use kit::{Camera, Control, Game, Heard, Image, Listener, Player, Screen, Sound, Timing};

    use super::*;
    use crate::motion::tests::grey;

    /// 鳴らしたものと止めた回数。
    #[derive(Default)]
    struct Log {
        begun: Vec<&'static str>,
        effects: Vec<&'static str>,
        silenced: usize,
        rested: usize,
        frames: usize,
    }
    type Shared = Rc<RefCell<Log>>;

    /// 曲は `song` だけ鳴る。お祝いは一瞬で終わる。
    struct Tape(Shared, Duration);
    impl Player for Tape {
        fn begin(&self, sound: Sound) -> Result<Timing> {
            assert_eq!(sound.game, crate::ID, "よその遊びの素材を鳴らした");
            self.0.borrow_mut().begun.push(sound.name);
            let total = if sound.name == "song" {
                self.1
            } else {
                Duration::ZERO
            };
            Ok(Timing {
                total,
                audible: total,
            })
        }
        fn effect(&self, sound: Sound) -> Result<()> {
            self.0.borrow_mut().effects.push(sound.name);
            Ok(())
        }
        fn silence(&self) {
            self.0.borrow_mut().silenced += 1;
        }
    }

    /// 灰色の絵を待たずに返す。
    struct Lens(Shared);
    impl Camera for Lens {
        fn frame(&mut self) -> Result<Image> {
            self.0.borrow_mut().frames += 1;
            std::thread::sleep(Duration::from_millis(5));
            Ok(grey())
        }
        fn rest(&mut self) {
            self.0.borrow_mut().rested += 1;
        }
    }

    struct Deaf;
    impl Listener for Deaf {
        fn hear(&mut self, _vocabulary: &[&str]) -> Result<Heard> {
            unreachable!("この遊びは聞かない")
        }
    }
    struct Blind;
    impl Screen for Blind {
        fn show(&mut self, _scene: Scene) {}
    }

    /// 始めてから `at` ミリ秒経ったら押す。それ以外は何も押さない。
    struct Presses {
        start: Instant,
        script: Vec<(u64, Press)>,
    }
    impl Control for Presses {
        fn poll(&mut self) -> Option<Press> {
            let now = self.start.elapsed().as_millis() as u64;
            let i = self.script.iter().position(|(at, _)| *at <= now)?;
            Some(self.script.remove(i).1)
        }
        fn wait(&mut self) -> Option<Press> {
            None
        }
    }

    fn devices(log: &Shared, song: Duration, script: Vec<(u64, Press)>) -> Devices {
        Devices {
            player: Box::new(Tape(log.clone(), song)),
            listener: Box::new(Deaf),
            screen: Box::new(Blind),
            control: Box::new(Presses {
                start: Instant::now(),
                script,
            }),
            camera: Box::new(Lens(log.clone())),
        }
    }

    /// 曲が鳴り終われば、お祝いをしてから返る。カメラは休ませる。
    #[test]
    fn the_song_ends_in_a_celebration() {
        let log = Shared::default();
        let mut dev = devices(&log, Duration::from_millis(150), vec![]);
        let mut game = BalloonCrash::new(&Config::default());
        assert_eq!(game.play(&mut dev).unwrap(), Ended::Finished);
        let log = log.borrow();
        assert_eq!(log.begun, ["song", "fanfare"]);
        assert!(log.frames > 3, "カメラを見ていない");
        assert!(log.rested >= 1, "カメラを休ませていない");
        assert_eq!(log.silenced, 0);
    }

    /// **「戻る」なら曲の途中でも止めて返る。** お祝いはしない。
    #[test]
    fn back_stops_midway() {
        let log = Shared::default();
        let mut dev = devices(&log, Duration::from_secs(60), vec![(100, Press::Back)]);
        let mut game = BalloonCrash::new(&Config::default());
        let t = Instant::now();
        assert_eq!(game.play(&mut dev).unwrap(), Ended::Finished);
        assert!(t.elapsed() < Duration::from_secs(5), "曲が終わるまで待った");
        let log = log.borrow();
        assert_eq!(log.begun, ["song"], "止めたのにお祝いをした");
        assert_eq!(log.silenced, 1);
        assert!(log.rested >= 1, "カメラを休ませていない");
    }

    /// **十字キーと決定では止まらない。** 押し間違いで遊びを切らない。
    #[test]
    fn other_buttons_do_not_stop() {
        let log = Shared::default();
        let mut dev = devices(
            &log,
            Duration::from_millis(150),
            vec![(10, Press::Ok), (20, Press::Left), (30, Press::Pick(0))],
        );
        let mut game = BalloonCrash::new(&Config::default());
        game.play(&mut dev).unwrap();
        assert_eq!(log.borrow().begun, ["song", "fanfare"]);
    }

    /// マウスで風船を押せば割れて、効果音が鳴る。割ったものは
    /// 「もう1回」の画面に並ぶ。
    #[test]
    fn a_poke_pops_and_sounds() {
        let log = Shared::default();
        // 風船が上がってくる頃から、画面じゅうを升目に区切って何度も押して回る。
        let grid: Vec<Press> = (0..10)
            .flat_map(|j| (0..12).map(move |i| Press::Point(50 + i * 100, 50 + j * 65)))
            .collect();
        let script = (0..20)
            .flat_map(|round| {
                let grid = grid.clone();
                grid.into_iter().map(move |p| (1000 + round * 50, p))
            })
            .collect();
        let mut dev = devices(&log, Duration::from_millis(2500), script);
        let mut game = BalloonCrash::new(&Config::default());
        game.play(&mut dev).unwrap();
        assert!(!log.borrow().effects.is_empty(), "割れた音が鳴っていない");
        assert!(!game.popped.is_empty());
        assert!(!game.backdrop().shapes.is_empty());
    }
}
