//! 進行。
//!
//! ```text
//! song 開始 ─→ ┌→ カメラの絵を1枚待つ（毎秒30枚くらい）
//!              │   動きの向きと速さを出す → 穴ごとに、つついたか・つかんで振り上げたかを見る
//!              │   流した・裏返した・乗った音などを鳴らす → 鏡と鉄板とたこ焼きを映す
//!              └── song が鳴り終わるまで（「戻る」なら途中で抜ける）
//!          ─→ fanfare（焼けた数と、ソースのかかったたこ焼きを並べる）
//! ```
//!
//! ここで返る。「もう1回」を訊くのは `home` の仕事。
//!
//! **時間切れという失敗は無い。** 1回の長さは曲の長さで決まり、終わりは
//! 必ずお祝いになる。

use std::time::{Duration, Instant};

use anyhow::Result;
use kit::{Devices, Ended, Press, Rect, Scene, Shape};
use vision::{Flow, Flowed, Seen};

use crate::config::Config;
use crate::cue::Cue;
use crate::look;
use crate::plate::{Event, Hole, Plate};

/// お祝いの間に画面を描き直す間隔。カメラを見ないので自分で刻む。
const TICK: Duration = Duration::from_millis(33);

/// 1周に鳴らす効果音の順。**効果音の口は1本なので、強いものを1つだけ選ぶ。**
const PRIORITY: [(Event, Cue); 7] = [
    (Event::Serve, Cue::Serve),
    (Event::Slip, Cue::Slip),
    (Event::Flip, Cue::Flip),
    (Event::Ready, Cue::Ready),
    (Event::Grab, Cue::Grab),
    (Event::Sizzle, Cue::Sizzle),
    (Event::Pour, Cue::Pour),
];

/// 穴1つぶんの、見えたもの。`dt` は前の絵からの秒数。
fn seen(flowed: &Flowed, h: &Hole, dt: f32) -> Seen {
    Seen::look(flowed, h.reach(), h.zones(), dt)
}

/// 起きたことから、鳴らす効果音を1つ選ぶ。
fn pick(events: &[Event]) -> Option<Cue> {
    PRIORITY
        .iter()
        .find(|(e, _)| events.contains(e))
        .map(|&(_, c)| c)
}

pub struct Takoyaki {
    cfg: Config,
    /// 前の回に焼けた数。「もう1回」の画面で後ろに敷く。
    served: usize,
}

impl Takoyaki {
    pub fn new(cfg: &Config) -> Self {
        Self {
            cfg: cfg.clone(),
            served: 0,
        }
    }

    /// 曲が終わるまで遊んで、お祝いまで。**カメラを休ませるのは呼び出し側**
    /// （どこで抜けても休ませるため）。
    fn round(&mut self, dev: &mut Devices, seed: u64) -> Result<Ended> {
        self.served = 0;
        let mut plate = Plate::new(&self.cfg, seed);
        let mut flow = Flow::default();

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
            let dt = now.saturating_sub(last).as_secs_f32();
            last = now;

            let mut events = Vec::new();
            while let Some(p) = dev.control.poll() {
                match p {
                    Press::Back => {
                        dev.player.silence();
                        self.served = plate.served();
                        return Ok(Ended::Finished);
                    }
                    // マウスで押しても進む。カメラが無いときに試す口。
                    Press::Point(x, y) => events.extend(plate.press(now, x, y)),
                    // 十字キーと決定では何も起きない。押し間違いで遊びを切らない。
                    _ => {}
                }
            }

            let flowed = flow.see(&mirror, self.cfg.threshold);
            events.extend(plate.step(now, |h| seen(&flowed, h, dt)));
            if let Some(cue) = pick(&events) {
                dev.player.effect(cue.sound())?;
            }
            dev.screen.show(look::playing(
                &mirror,
                &plate,
                &flowed,
                now,
                self.cfg.show_motion,
            ));
        }

        self.served = plate.served();
        dev.camera.rest();
        let fanfare = dev.player.begin(Cue::Fanfare.sound())?;
        let start = Instant::now();
        while start.elapsed() < fanfare.total {
            if dev.control.stop_requested() {
                dev.player.silence();
                return Ok(Ended::Finished);
            }
            dev.screen.show(look::finale(self.served, start.elapsed()));
            std::thread::sleep(TICK);
        }
        Ok(Ended::Finished)
    }
}

impl kit::Game for Takoyaki {
    fn id(&self) -> &'static str {
        crate::ID
    }

    fn title(&self) -> &'static str {
        "たこやき くるりん！"
    }

    fn glyphs(&self) -> &'static str {
        look::GLYPHS
    }

    fn sounds(&self) -> &'static [&'static str] {
        &crate::SOUNDS
    }

    fn icon(&self, area: Rect) -> Vec<Shape> {
        look::icon(area)
    }

    fn backdrop(&self) -> Scene {
        look::finale(self.served, Duration::ZERO)
    }

    fn play(&mut self, dev: &mut Devices) -> Result<Ended> {
        let result = self.round(dev, rand::random());
        // **どこで抜けてもカメラを休ませる。** ランプが点いたままだと、
        // 次に選ぶ遊びの間も撮られているように見える。
        dev.camera.rest();
        result
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use kit::{Camera, Control, Game, Heard, Image, Listener, Player, Screen, Sound, Timing};
    use vision::sample::{blob, grey};

    use super::*;
    use crate::plate::{Stage, HOLES};

    #[derive(Default)]
    struct Log {
        begun: Vec<&'static str>,
        effects: Vec<&'static str>,
        silenced: usize,
        rested: usize,
        frames: usize,
    }
    type Shared = Rc<RefCell<Log>>;

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
    struct Screens(Arc<AtomicUsize>);
    impl Screen for Screens {
        fn show(&mut self, _scene: Scene) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

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
            screen: Box::new(Screens(Arc::default())),
            control: Box::new(Presses {
                start: Instant::now(),
                script,
            }),
            camera: Box::new(Lens(log.clone())),
        }
    }

    fn config() -> Config {
        Config {
            cook_min: 1.0,
            cook_max: 1.2,
            ..Config::default()
        }
    }

    #[test]
    fn the_song_ends_in_a_celebration() {
        let log = Shared::default();
        let mut dev = devices(&log, Duration::from_millis(150), vec![]);
        let mut game = Takoyaki::new(&config());
        assert_eq!(game.play(&mut dev).unwrap(), Ended::Finished);
        let log = log.borrow();
        assert_eq!(log.begun, ["song", "fanfare"]);
        assert!(log.frames > 3);
        assert!(log.rested >= 1, "カメラを休ませていない");
    }

    #[test]
    fn back_stops_midway() {
        let log = Shared::default();
        let mut dev = devices(&log, Duration::from_secs(60), vec![(100, Press::Back)]);
        let mut game = Takoyaki::new(&config());
        let t = Instant::now();
        game.play(&mut dev).unwrap();
        assert!(t.elapsed() < Duration::from_secs(5));
        let log = log.borrow();
        assert_eq!(log.begun, ["song"]);
        assert_eq!(log.silenced, 1);
        assert!(log.rested >= 1);
    }

    /// マウスで穴を押し続ければ、裏返って舟皿へ乗る。音も鳴る。
    #[test]
    fn pressing_cooks_flips_and_serves() {
        let log = Shared::default();
        let script = (0..60)
            .flat_map(|k| {
                HOLES
                    .iter()
                    .map(move |&(x, y)| (300 + k * 100, Press::Point(x as i32, y as i32)))
            })
            .collect();
        let mut dev = devices(&log, Duration::from_millis(6000), script);
        let mut game = Takoyaki::new(&config());
        game.play(&mut dev).unwrap();
        let log = log.borrow();
        for cue in ["pour", "flip", "serve"] {
            assert!(
                log.effects.contains(&cue),
                "{cue} が鳴っていない: {:?}",
                log.effects
            );
        }
        assert!(game.served > 0);
    }

    /// カメラの絵から穴の判定まで、通しで。指を当てると裏返り、つかんで
    /// 振り上げると舟皿へ飛ぶ。
    #[test]
    fn a_finger_flips_and_a_swing_serves_through_the_camera() {
        let mut plate = Plate::new(&config(), 3);
        let mut flow = Flow::default();
        let dt = 1.0 / 30.0;
        let mut t = Duration::ZERO;
        let (hx, hy) = (HOLES[1].0 as i32, HOLES[1].1 as i32);
        let mut step = |plate: &mut Plate, t: &mut Duration, img: &Image| {
            *t += Duration::from_secs_f32(dt);
            let flowed = flow.see(img, 24);
            plate.step(*t, |h| seen(&flowed, h, dt))
        };
        // 片面が焼けるまで待つ。
        while !matches!(plate.holes()[1].stage, Stage::ReadyA { .. }) {
            step(&mut plate, &mut t, &grey());
            assert!(t < Duration::from_secs(10));
        }
        // 指で突っつく（揺らした白い丸を当てる）。
        let mut events = Vec::new();
        for i in 0..10 {
            let x = hx + ((i as f32 * 1.7).sin() * 20.0) as i32;
            events.extend(step(&mut plate, &mut t, &blob(x, hy, 40)));
        }
        assert!(events.contains(&Event::Flip), "裏返らない: {events:?}");
        // 反対側が焼けるまで、手を離して待つ。
        while !matches!(plate.holes()[1].stage, Stage::ReadyB { .. }) {
            step(&mut plate, &mut t, &grey());
            assert!(t < Duration::from_secs(20));
        }
        // 手を入れてつかみ、上へ振り上げる。
        let mut events = Vec::new();
        for i in 0..40 {
            let k = i as f32 * dt;
            let wobble = ((k * 25.0).sin() * 12.0) as i32;
            let up = ((k - 0.5).max(0.0) * 700.0) as i32;
            events.extend(step(
                &mut plate,
                &mut t,
                &blob(hx - 60 + wobble, hy - up, 55),
            ));
            if events.contains(&Event::Serve) {
                break;
            }
        }
        assert!(events.contains(&Event::Grab), "つかめない: {events:?}");
        assert!(events.contains(&Event::Serve), "舟皿へ飛ばない: {events:?}");
    }

    #[test]
    fn the_strongest_event_gets_the_sound() {
        assert_eq!(
            pick(&[Event::Pour, Event::Serve, Event::Flip]),
            Some(Cue::Serve)
        );
        assert_eq!(pick(&[Event::Sizzle, Event::Ready]), Some(Cue::Ready));
        assert_eq!(pick(&[]), None);
    }
}
