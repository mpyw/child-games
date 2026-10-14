//! カメラから撮る。nokhwa を使う。
//!
//! macOS では AVFoundation、ラズパイでは V4L2 を使う。どちらも USB の
//! Web カメラと内蔵カメラで動く。
//!
//! **撮るのは専用のスレッド。** nokhwa の `Camera` はスレッドを跨げないうえ、
//! 1枚待つと 30ms 塞がる。遊びのスレッドで撮ると、そのぶん「戻る」を
//! 見るのが遅れる。絵はチャンネルで受け取り、溜まっていれば最新だけ使う。

use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError, TrySendError};
use std::time::Duration;

use anyhow::{Context, Result};
use kit::{Camera, Image};
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    ApiBackend, CameraFormat, CameraInfo, FrameFormat, RequestedFormat, RequestedFormatType,
    Resolution,
};

use super::shrink;
use crate::config::Config;

/// 最初の1枚を待つ上限。開いた直後は露出合わせで遅れる。
const FIRST: Duration = Duration::from_secs(5);

/// 撮る係への頼み。
enum Command {
    /// 撮り始める。
    Roll,
    /// 撮るのをやめる。ランプが消える。
    Rest,
}

pub struct Webcam {
    commands: Sender<Command>,
    frames: Receiver<Result<Image, String>>,
    rolling: bool,
}

impl Webcam {
    /// **起動時に開く。カメラが無ければここで止める。** 選んでから気づくと、
    /// テレビの前で遊びが始まらない。
    ///
    /// 開くだけで、撮り始めるのは遊びが最初に `frame` を呼んだとき。
    /// カメラを使わない遊びの間はランプが点かない。
    pub fn open(cfg: &Config) -> Result<Self> {
        let want = cfg.camera.device().map(str::to_owned);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (commands, command_rx) = mpsc::channel();
        // 2枚まで溜める。遊びが追いつかないぶんは捨てる（最新だけ要る）。
        let (frame_tx, frames) = mpsc::sync_channel(2);
        std::thread::Builder::new()
            .name("camera".into())
            .spawn(move || {
                let cam = match connect(want.as_deref()) {
                    Ok(cam) => {
                        let _ = ready_tx.send(Ok(()));
                        cam
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                serve(cam, command_rx, frame_tx);
            })
            .context("カメラのスレッドを立てられない")?;
        ready_rx
            .recv()
            .context("カメラのスレッドが何も言わずに死んだ")??;
        Ok(Self {
            commands,
            frames,
            rolling: false,
        })
    }

    /// 溜まっているぶんを捨てる。前の回の最後の絵を、次の回の最初の絵と
    /// 比べると、全面が動いたように見える。
    fn drain(&self) {
        while self.frames.try_recv().is_ok() {}
    }
}

impl Camera for Webcam {
    fn frame(&mut self) -> Result<Image> {
        if !self.rolling {
            self.drain();
            self.commands
                .send(Command::Roll)
                .context("カメラのスレッドが死んでいる")?;
            self.rolling = true;
        }
        let mut got = self
            .frames
            .recv_timeout(FIRST)
            .context("カメラから絵が来ない")?;
        while let Ok(newer) = self.frames.try_recv() {
            got = newer;
        }
        got.map_err(anyhow::Error::msg)
    }

    fn rest(&mut self) {
        if self.rolling {
            let _ = self.commands.send(Command::Rest);
            self.rolling = false;
        }
    }
}

/// カメラを選んで開く。撮り始めはしない。
fn connect(want: Option<&str>) -> Result<nokhwa::Camera> {
    // **macOS では許可を先に取る。** 取らずに開くと、許可の窓が出ている
    // 間に開くのが失敗する。他の OS ではすぐ `true` が返る。
    let (tx, rx) = mpsc::channel();
    nokhwa::nokhwa_initialize(move |granted| {
        let _ = tx.send(granted);
    });
    let granted = rx.recv_timeout(Duration::from_secs(60)).unwrap_or(false);
    anyhow::ensure!(
        granted,
        "カメラを使う許可が無い（システム設定 → プライバシーとセキュリティ → カメラ）"
    );

    let cams = nokhwa::query(ApiBackend::Auto).context("カメラを探せない")?;
    let names: Vec<String> = cams.iter().map(CameraInfo::human_name).collect();
    anyhow::ensure!(!cams.is_empty(), "カメラが見つからない");
    let info = choose(&cams, want);
    eprintln!(
        "  カメラ {}（見えているもの: {}）",
        info.human_name(),
        names.join(", ")
    );

    // **640×480 から当たるものを探す。** 縮めて 320×180 にするので、
    // それ以上は復号の手間が増えるだけ。ラズパイでは効く。形式を決め打ち
    // にすると、持っていないカメラで開けなくなるので、順に試して最後は
    // 何でもよいにする。
    let wants = [
        RequestedFormatType::Closest(CameraFormat::new(
            Resolution::new(640, 480),
            FrameFormat::MJPEG,
            30,
        )),
        RequestedFormatType::Closest(CameraFormat::new(
            Resolution::new(640, 480),
            FrameFormat::YUYV,
            30,
        )),
        RequestedFormatType::AbsoluteHighestFrameRate,
    ];
    let mut last = None;
    for w in wants {
        match nokhwa::Camera::new(info.index().clone(), RequestedFormat::new::<RgbFormat>(w)) {
            Ok(cam) => {
                let f = cam.camera_format();
                eprintln!(
                    "  カメラの形式 {}×{} {:?} {}fps",
                    f.width(),
                    f.height(),
                    f.format(),
                    f.frame_rate()
                );
                return Ok(cam);
            }
            Err(e) => last = Some(e),
        }
    }
    Err(anyhow::anyhow!("カメラを開けない: {}", info.human_name()))
        .context(last.map(|e| e.to_string()).unwrap_or_default())
}

/// 名前の一部で選ぶ。**見つからなければ警告して先頭を使う。** マイクと
/// 同じで、テレビに繋ぎっぱなしの玩具は起動しないほうが困る。
fn choose<'a>(cams: &'a [CameraInfo], want: Option<&str>) -> &'a CameraInfo {
    let Some(want) = want else {
        return &cams[0];
    };
    match cams.iter().find(|c| c.human_name().contains(want)) {
        Some(c) => c,
        None => {
            eprintln!(
                "  ⚠ カメラ「{want}」が見つからないので {} を使う",
                cams[0].human_name()
            );
            &cams[0]
        }
    }
}

/// 頼まれたら撮って送る。遊びの側が落ちたら抜ける。
fn serve(
    mut cam: nokhwa::Camera,
    commands: Receiver<Command>,
    frames: SyncSender<Result<Image, String>>,
) {
    loop {
        match commands.recv() {
            Ok(Command::Roll) => {}
            Ok(Command::Rest) => continue,
            Err(_) => return,
        }
        if let Err(e) = cam.open_stream() {
            let _ = frames.send(Err(format!("カメラで撮り始められない: {e}")));
            continue;
        }
        loop {
            match commands.try_recv() {
                Ok(Command::Rest) => break,
                Ok(Command::Roll) | Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    let _ = cam.stop_stream();
                    return;
                }
            }
            let shot = cam
                .frame()
                .and_then(|buf| buf.decode_image::<RgbFormat>())
                .map(|img| {
                    shrink(img.width() as usize, img.height() as usize, |x, y| {
                        let p = img.get_pixel(x as u32, y as u32).0;
                        (p[0], p[1], p[2])
                    })
                })
                .map_err(|e| format!("カメラの絵を読めない: {e}"));
            // **読めないときは間を置く。** 抜かれたカメラは即座に失敗を
            // 返し続けるので、そのまま回すと1コアを食い切る。
            if shot.is_err() {
                std::thread::sleep(Duration::from_millis(100));
            }
            match frames.try_send(shot) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
        let _ = cam.stop_stream();
    }
}
