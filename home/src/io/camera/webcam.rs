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
    eprintln!("  カメラの候補: {}", names.join(", "));

    // **開けるものが見つかるまで順に試す。** 1台のカメラが複数の口を
    // 生やすことがある。ラズパイの V15AF は `/dev/video0`（絵）と
    // `/dev/video1`（付帯情報）の2つを同じ名前で出し、後者は開けない。
    // 名前で1つに決め打ちしていた頃は、そちらを掴んで起動ごと落ちた。
    let mut failed = Vec::new();
    for i in candidates(&names, want) {
        let info = &cams[i];
        match open(info) {
            Ok(cam) => {
                let f = cam.camera_format();
                eprintln!(
                    "  カメラ {}（{}）{}×{} {:?} {}fps",
                    info.human_name(),
                    info.index(),
                    f.width(),
                    f.height(),
                    f.format(),
                    f.frame_rate()
                );
                return Ok(cam);
            }
            Err(e) => failed.push(format!("{}（{}）: {e}", info.human_name(), info.index())),
        }
    }
    anyhow::bail!("カメラを開けない\n    {}", failed.join("\n    "))
}

/// 1つのカメラを開く。撮り始めはしない。
///
/// **640×480 から当たるものを探す。** 縮めて 320×180 にするので、
/// それ以上は復号の手間が増えるだけ。ラズパイでは効く。形式を決め打ち
/// にすると、持っていないカメラで開けなくなるので、順に試して最後は
/// 何でもよいにする。
fn open(info: &CameraInfo) -> Result<nokhwa::Camera, nokhwa::NokhwaError> {
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
            Ok(cam) => return Ok(cam),
            Err(e) => last = Some(e),
        }
    }
    Err(last.expect("頼む形式は空ではない"))
}

/// 試す順。名前の一部で選ぶなら、当たるものだけを並んだ順に。
///
/// **当たるものが無ければ警告して全部を試す。** マイクと同じで、テレビに
/// 繋ぎっぱなしの玩具は起動しないほうが困る。
fn candidates(names: &[String], want: Option<&str>) -> Vec<usize> {
    let all = (0..names.len()).collect();
    let Some(want) = want else {
        return all;
    };
    let hits: Vec<usize> = (0..names.len())
        .filter(|&i| names[i].contains(want))
        .collect();
    if hits.is_empty() {
        eprintln!("  ⚠ カメラ「{want}」が見つからないので、見えているものを順に試す");
        return all;
    }
    hits
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

#[cfg(test)]
mod tests {
    use super::candidates;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// **同じ名前が2つあれば、両方試す。** 開けない口を掴んで落ちないように。
    #[test]
    fn every_camera_with_the_name_is_a_candidate() {
        let n = names(&["V15AF", "V15AF", "pispbe-input", "rpi-hevc-dec"]);
        assert_eq!(candidates(&n, Some("V15AF")), [0, 1]);
    }

    /// 名前が無ければ、見えている順に全部。
    #[test]
    fn no_name_means_everything_in_order() {
        let n = names(&["MacBook Pro Camera", "Desk View"]);
        assert_eq!(candidates(&n, None), [0, 1]);
    }

    /// 当たらない名前でも起動は止めない。全部を試す。
    #[test]
    fn an_unknown_name_falls_back_to_everything() {
        let n = names(&["MacBook Pro Camera"]);
        assert_eq!(candidates(&n, Some("V15AF")), [0]);
    }
}
