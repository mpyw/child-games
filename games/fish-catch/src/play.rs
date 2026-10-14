//! 進行。
//!
//! ```text
//! song 開始 ─→ ┌→ カメラの絵を1枚待つ（毎秒30枚くらい）
//!              │   動きの向きと速さを出す → 魚ごとに、つかんだか・振り上げたかを見る
//!              │   釣れた・逃げた・つかんだ音を鳴らす → 鏡と池と魚を映す
//!              └── song が鳴り終わるまで（「戻る」なら途中で抜ける）
//!          ─→ fanfare（釣った数と、釣った魚を並べる）
//! ```
//!
//! ここで返る。「もう1回」を訊くのは `home` の仕事。
//!
//! **時間切れという失敗は無い。** 1回の長さは曲の長さで決まり、終わりは
//! 必ずお祝いになる。

use std::time::{Duration, Instant};

use anyhow::Result;
use kit::{Devices, Ended, Press, Rect, Scene, Shape};
use vision::{Flow, Flowed, Region};

use crate::config::Config;
use crate::cue::Cue;
use crate::look;
use crate::pond::{Creature, Event, Fish, Pond, Seen};

/// お祝いの間に画面を描き直す間隔。カメラを見ないので自分で刻む。
const TICK: Duration = Duration::from_millis(33);

/// 向きが分かった升目がこれより少なければ、振り上げは無かったことにする。
/// 1つ2つの升目は、カメラのざらつきでも向きを持つ。
const MIN_VECTORS: usize = 6;

/// 魚1匹ぶんの、見えたもの。`dt` は前の絵からの秒数。
fn seen(flowed: &Flowed, f: &Fish, dt: f32) -> Seen {
    let speed = |r: Region| {
        if r.vectors >= MIN_VECTORS {
            r.up / dt
        } else {
            0.0
        }
    };
    let (x, y, r) = f.reach();
    let (left, right) = f.zones();
    Seen {
        grab: flowed.ratio_in(x, y, r),
        left: speed(flowed.region(left)),
        right: speed(flowed.region(right)),
    }
}

pub struct FishCatch {
    cfg: Config,
    /// 前の回に釣った魚。「もう1回」の画面で後ろに敷く。
    caught: Vec<Creature>,
}

impl FishCatch {
    pub fn new(cfg: &Config) -> Self {
        Self {
            cfg: cfg.clone(),
            caught: Vec::new(),
        }
    }

    /// 曲が終わるまで遊んで、お祝いまで。**カメラを休ませるのは呼び出し側**
    /// （どこで抜けても休ませるため）。
    fn round(&mut self, dev: &mut Devices, seed: u64) -> Result<Ended> {
        self.caught.clear();
        let mut pond = Pond::new(&self.cfg, seed);
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
            // 速さは「前の絵から何画素」なので、絵の間の時間で割って毎秒にする。
            // 最初の1枚や、詰まって2枚続けて来たときに割りすぎないよう下限を置く。
            let dt = now.saturating_sub(last).as_secs_f32().max(1.0 / 60.0);
            last = now;

            let mut events = Vec::new();
            while let Some(p) = dev.control.poll() {
                match p {
                    Press::Back => {
                        dev.player.silence();
                        self.caught = pond.haul();
                        return Ok(Ended::Finished);
                    }
                    // マウスで押しても釣れる。カメラが無いときに試す口。
                    Press::Point(x, y) if pond.poke(now, x, y) => events.push(Event::Catch),
                    // 十字キーと決定では何も起きない。押し間違いで遊びを切らない。
                    _ => {}
                }
            }

            let flowed = flow.see(&mirror, self.cfg.threshold);
            events.extend(pond.step(now, dt, |f| seen(&flowed, f, dt)));
            // **1周に1つだけ鳴らす。** 効果音の口は1本なので、強いものを選ぶ。
            let cue = [Event::Catch, Event::Slip, Event::Grab]
                .into_iter()
                .find(|e| events.contains(e))
                .map(|e| match e {
                    Event::Catch => Cue::Catch,
                    Event::Slip => Cue::Slip,
                    Event::Grab => Cue::Grab,
                });
            if let Some(cue) = cue {
                dev.player.effect(cue.sound())?;
            }

            let motion = self.cfg.show_motion.then_some(&flowed);
            dev.screen.show(look::playing(&mirror, &pond, now, motion));
        }

        self.caught = pond.haul();
        dev.camera.rest();
        let fanfare = dev.player.begin(Cue::Fanfare.sound())?;
        let start = Instant::now();
        while start.elapsed() < fanfare.total {
            if dev.control.stop_requested() {
                dev.player.silence();
                return Ok(Ended::Finished);
            }
            dev.screen.show(look::finale(&self.caught, start.elapsed()));
            std::thread::sleep(TICK);
        }
        Ok(Ended::Finished)
    }
}

impl kit::Game for FishCatch {
    fn id(&self) -> &'static str {
        crate::ID
    }

    fn title(&self) -> &'static str {
        "おさかなキャッチ！"
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
        look::finale(&self.caught, Duration::ZERO)
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

    /// 決めた絵を順に返す。尽きたら灰色。
    struct Lens(Shared, Vec<Image>);
    impl Camera for Lens {
        fn frame(&mut self) -> Result<Image> {
            let n = {
                let mut l = self.0.borrow_mut();
                l.frames += 1;
                l.frames
            };
            std::thread::sleep(Duration::from_millis(5));
            Ok(self.1.get(n - 1).cloned().unwrap_or_else(grey))
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
    /// 映した数を数える。`Screen` は `Send` を要るので、数は原子的に持つ。
    struct Screens(Arc<AtomicUsize>);
    impl Screen for Screens {
        fn show(&mut self, _scene: Scene) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 始めてから `at` ミリ秒経ったら押す。
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

    fn devices(
        log: &Shared,
        scenes: &Arc<AtomicUsize>,
        song: Duration,
        frames: Vec<Image>,
        script: Vec<(u64, Press)>,
    ) -> Devices {
        Devices {
            player: Box::new(Tape(log.clone(), song)),
            listener: Box::new(Deaf),
            screen: Box::new(Screens(scenes.clone())),
            control: Box::new(Presses {
                start: Instant::now(),
                script,
            }),
            camera: Box::new(Lens(log.clone(), frames)),
        }
    }

    /// カメラの絵から魚の判定まで、通しで。`hands` は魚の位置と時刻から、
    /// 左右の手の位置を返す。釣れたか・逃げたかを返す。
    fn act(hands: impl Fn(f32, f32, f32) -> [(i32, i32); 2]) -> Vec<Event> {
        act_at(30.0, hands)
    }

    /// `act` を、カメラが毎秒 `fps` 枚のときで。
    fn act_at(fps: f32, hands: impl Fn(f32, f32, f32) -> [(i32, i32); 2]) -> Vec<Event> {
        let cfg = Config {
            max_fish: 1,
            ..Config::default()
        };
        let mut pond = Pond::new(&cfg, 21);
        let mut flow = Flow::default();
        let dt = 1.0 / fps;
        let mut t = Duration::ZERO;
        // 魚が画面の真ん中あたりまで泳いでくるのを待つ。
        while pond.fish().is_empty() || !(300.0..980.0).contains(&pond.fish()[0].x) {
            t += Duration::from_secs_f32(dt);
            pond.step(t, dt, |_| Seen::default());
        }
        let mut events = Vec::new();
        for i in 0..(fps * 2.0) as usize {
            t += Duration::from_secs_f32(dt);
            let (fx, fy) = (pond.fish()[0].x, pond.fish()[0].y);
            let [l, r] = hands(fx, fy, i as f32 * dt);
            let img = vision::sample::blobs(&[(l.0, l.1, 55), (r.0, r.1, 55)]);
            let flowed = flow.see(&img, cfg.threshold);
            events.extend(pond.step(t, dt, |f| seen(&flowed, f, dt)));
            if events.contains(&Event::Catch) || events.contains(&Event::Slip) {
                break;
            }
        }
        events
    }

    /// 手を魚の左右に入れて少し揺らし、`after` 秒から上へ振り上げる。
    /// 速さは左右それぞれ毎秒の画素。
    fn lift(left: f32, right: f32) -> impl Fn(f32, f32, f32) -> [(i32, i32); 2] {
        move |fx, fy, t| {
            let after = 0.5;
            let wobble = ((t * 25.0).sin() * 12.0) as i32;
            let up = |speed: f32| ((t - after).max(0.0) * speed) as i32;
            [
                (fx as i32 - 110 + wobble, fy as i32 - up(left)),
                (fx as i32 + 110 - wobble, fy as i32 - up(right)),
            ]
        }
    }

    /// 両手で勢いよく振り上げたら釣れる。カメラの絵から通しで。
    #[test]
    fn both_hands_swung_up_catch_through_the_camera() {
        let ev = act(lift(750.0, 750.0));
        assert!(ev.contains(&Event::Grab), "つかめていない: {ev:?}");
        assert!(ev.contains(&Event::Catch), "釣れていない: {ev:?}");
    }

    /// **ラズパイのカメラ（毎秒13枚）でも、勢いよく振り上げれば釣れる。**
    /// 1枚あたりのずれが大きくなるので、探す幅が足りないと取りこぼす。
    #[test]
    fn a_fast_swing_catches_even_with_a_slow_camera() {
        let ev = act_at(13.0, lift(900.0, 900.0));
        assert!(ev.contains(&Event::Catch), "釣れていない: {ev:?}");
    }

    /// **片手だけでも、勢いよく振り上げれば釣れる。** カメラの絵から通しで。
    #[test]
    fn one_hand_swung_up_catches_through_the_camera() {
        let ev = act(lift(750.0, 0.0));
        assert!(ev.contains(&Event::Catch), "釣れていない: {ev:?}");
    }

    /// ゆっくり持ち上げると逃げられる。カメラの絵から通しで。
    #[test]
    fn a_slow_lift_slips_through_the_camera() {
        let ev = act(lift(200.0, 200.0));
        assert!(ev.contains(&Event::Slip), "逃げていない: {ev:?}");
    }

    /// 曲が鳴り終われば、お祝いをしてから返る。カメラは休ませる。
    #[test]
    fn the_song_ends_in_a_celebration() {
        let log = Shared::default();
        let scenes = Arc::default();
        let mut dev = devices(&log, &scenes, Duration::from_millis(150), vec![], vec![]);
        let mut game = FishCatch::new(&Config::default());
        assert_eq!(game.play(&mut dev).unwrap(), Ended::Finished);
        let log = log.borrow();
        assert_eq!(log.begun, ["song", "fanfare"]);
        assert!(log.frames > 3 && scenes.load(Ordering::Relaxed) > 3);
        assert!(log.rested >= 1, "カメラを休ませていない");
    }

    /// **「戻る」なら曲の途中でも止めて返る。** お祝いはしない。
    #[test]
    fn back_stops_midway() {
        let log = Shared::default();
        let scenes = Arc::default();
        let mut dev = devices(
            &log,
            &scenes,
            Duration::from_secs(60),
            vec![],
            vec![(100, Press::Back)],
        );
        let mut game = FishCatch::new(&Config::default());
        let t = Instant::now();
        game.play(&mut dev).unwrap();
        assert!(t.elapsed() < Duration::from_secs(5), "曲が終わるまで待った");
        let log = log.borrow();
        assert_eq!(log.begun, ["song"]);
        assert_eq!(log.silenced, 1);
        assert!(log.rested >= 1);
    }

    /// マウスで魚を押せば釣れて、釣れた音が鳴り、釣った数に入る。
    #[test]
    fn a_poke_catches_and_sounds() {
        let log = Shared::default();
        let grid: Vec<Press> = (0..8)
            .flat_map(|j| (0..13).map(move |i| Press::Point(i * 100, 380 + j * 40)))
            .collect();
        let script = (0..20)
            .flat_map(|round| {
                let grid = grid.clone();
                grid.into_iter().map(move |p| (1500 + round * 50, p))
            })
            .collect();
        let scenes = Arc::default();
        let mut dev = devices(&log, &scenes, Duration::from_millis(3500), vec![], script);
        let mut game = FishCatch::new(&Config::default());
        game.play(&mut dev).unwrap();
        assert!(
            log.borrow().effects.contains(&"catch"),
            "釣れた音が鳴っていない"
        );
        assert!(!game.caught.is_empty());
    }

    /// 動きの向きを画面に出す設定でも回る。
    #[test]
    fn showing_the_motion_does_not_break() {
        let log = Shared::default();
        // 白い丸が上へ動き続ける絵。
        let frames = (0..40).map(|i| blob(640, 600 - i * 8, 80)).collect();
        let scenes = Arc::default();
        let mut dev = devices(&log, &scenes, Duration::from_millis(300), frames, vec![]);
        let mut game = FishCatch::new(&Config {
            show_motion: true,
            ..Config::default()
        });
        game.play(&mut dev).unwrap();
        assert!(scenes.load(Ordering::Relaxed) > 3);
    }
}
