//! 遊びを並べて、テレビの前で選ばせる。
//!
//!     cargo run                             マイク＋ウィンドウ
//!     cargo run -- --terminal               ウィンドウを使わず文字だけ
//!     cargo run -- --keyboard               マイクの代わりに手打ち
//!     cargo run -- --game donna-iro         メニューを飛ばしてその遊びから
//!     cargo run -- --once                   1回遊んだら終わる（もう1回を待たない）
//!     cargo run -- --config other.toml      別の設定ファイルを読む
//!     cargo run -- --help                   使い方
//!     cargo run --no-default-features       whisper もウィンドウもカメラも無し
//!
//! 設定は `config.toml` に集約してある。何をいじれるかはそのファイルを見ること。

mod config;
mod io;
mod menu;

use anyhow::{Context, Result};
use kit::{Control, Devices, Game, Listener, Screen};

struct Options {
    keyboard: bool,
    terminal: bool,
    once: bool,
    /// メニューを飛ばして始める遊び（`catalog` の何番目か）。
    start: Option<usize>,
    config: config::Config,
}

/// メニューに並べる遊び。**遊びを足したら、ここと `Cargo.toml` と
/// `config::Games` に書く。** 並べた順にメニューに出る。
fn catalog(cfg: &config::Config) -> Vec<Box<dyn Game>> {
    vec![
        Box::new(donna_iro::DonnaIro::new(&cfg.games.donna_iro)),
        Box::new(balloon_crash::BalloonCrash::new(&cfg.games.balloon_crash)),
    ]
}

/// 引数だけを読んだ結果。**設定の読み込みはここに混ぜない。**
/// ファイルにも環境にも触らないので、そのまま試せる。
#[derive(Debug, PartialEq)]
struct Flags {
    keyboard: bool,
    terminal: bool,
    once: bool,
    help: bool,
    game: Option<String>,
    config: Option<std::path::PathBuf>,
}

/// 使い方。`--help` で出す。
///
/// **遊びの名前は `catalog` から引く。** 手で並べると、遊びを足したときに
/// 書き忘れる。
fn usage() -> String {
    let ids: Vec<&str> = catalog(&config::Config::default())
        .iter()
        .map(|g| g.id())
        .collect();
    format!(
        "\
あそびをえらぶ

  --keyboard          マイクの代わりに手打ち
  --terminal          ウィンドウを使わず文字だけ
  --game <name>       メニューを飛ばしてその遊びから（{}）
  --once              1回遊んだら終わる（もう1回を待たない）
  --config <path>     別の設定ファイルを読む
  --help              これ

設定は config.toml に集約してある。何をいじれるかはそのファイルを見ること。",
        ids.join(" / ")
    )
}

/// 引数を順に食べる。**知らないものは黙って捨てない。**
///
/// 拾い読みしていた頃は `--termnal` のようなタイポが黙ってウィンドウで
/// 起動し、`--config` の値が無ければ既定に落ちた。指定したのに効いて
/// いない、が一番たちが悪い。
fn parse_flags(args: impl Iterator<Item = String>) -> Result<Flags> {
    let mut flags = Flags {
        keyboard: false,
        terminal: cfg!(not(feature = "window")),
        once: false,
        help: false,
        game: None,
        config: None,
    };

    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--keyboard" => flags.keyboard = true,
            "--terminal" => flags.terminal = true,
            "--once" => flags.once = true,
            // 知らない引数として弾くと exit 1 になる。訊いただけで
            // 失敗にするのは行儀が悪い。
            "--help" | "-h" => flags.help = true,
            "--config" => {
                let path = args.next().context("--config にファイル名が無い")?;
                flags.config = Some(std::path::PathBuf::from(path));
            }
            "--game" => flags.game = Some(args.next().context("--game に遊びの名前が無い")?),
            other => anyhow::bail!("知らない引数: {other}\n\n{}", usage()),
        }
    }
    Ok(flags)
}

fn parse_args() -> Result<Options> {
    let flags = parse_flags(std::env::args().skip(1))?;
    if flags.help {
        println!("{}", usage());
        std::process::exit(0);
    }
    let config = config::load(flags.config.as_deref())?;
    // **知らない名前は起動時に弾く。** テレビの前で黙ってメニューが出ると、
    // 起動設定のタイプミスに気づけない。
    let start = match &flags.game {
        None => None,
        Some(name) => {
            let ids: Vec<&str> = catalog(&config).iter().map(|g| g.id()).collect();
            let i = ids
                .iter()
                .position(|id| id == name)
                .with_context(|| format!("知らない遊び: {name}（あるのは {}）", ids.join(", ")))?;
            Some(i)
        }
    };
    Ok(Options {
        keyboard: flags.keyboard,
        terminal: flags.terminal,
        once: flags.once,
        start,
        config,
    })
}

fn main() -> Result<()> {
    let opts = parse_args()?;

    // 在処を決めて、揃っているかまで見る。遊んでいる最中に落ちないように。
    // **並べる遊びは全部見る。** 選ばれてから見るのでは遅い。
    let sounds: Vec<_> = catalog(&opts.config)
        .iter()
        .map(|g| (g.id(), g.sounds()))
        .collect();
    io::audio::configure(&opts.config, &sounds)?;

    if opts.terminal {
        return play(
            opts,
            Box::new(io::screen::Terminal),
            Box::new(io::control::Stdin),
        );
    }

    #[cfg(feature = "window")]
    {
        // ウィンドウはメインスレッドに置く必要がある。しかも cpal も
        // rodio も !Send なので、ゲーム側をワーカースレッドに出して
        // そこで音の口を開く。
        let (tx, rx) = std::sync::mpsc::channel();
        // 逆向きに1本。押されたもの（`kit::Press`）を運ぶ。
        let (press_tx, press_rx) = std::sync::mpsc::channel();
        // テレビのリモコン（CEC）からも受ける。**送り手が1本増えるだけ**で、
        // 受け口から下は何も変わらない。
        //
        // minifb では受けられない。Bravia の決定ボタンは KEY_OK で来て、
        // `minifb::Key` に OK が無いため一生 Space/Enter にならない。
        match io::control::watch_remote(&press_tx) {
            0 => eprintln!("  リモコン なし（マウスとキーボードで操作）"),
            n => eprintln!("  リモコン {n}台ぶん見張る"),
        }
        let control: Box<dyn Control> = Box::new(io::control::Channel(press_rx));
        // ゲーム側が落ちたことを main へ伝える口。**これが無いと、音源や
        // デバイスの障害で死んでも終了コードが0になる。** systemd の
        // 再起動条件や起動スクリプトからは成功に見えてしまう。
        let (dead_tx, dead_rx) = std::sync::mpsc::channel();
        // フレームの口をもう1本、ワーカーに握らせたままにする。
        //
        // **これが無いと結果が間に合わない。** `play` が返る時点でその中の
        // `Remote` が落ち、ウィンドウ側は送信側の切断を見てすぐ抜ける。
        // 下の `try_recv` がそのあと走るので、`dead_tx.send` と競走になる。
        // 最後の送信側をここに残しておけば、送ってから畳む順序になる。
        let alive = tx.clone();
        std::thread::spawn(move || {
            // **パニックしても必ず報せる。** 巻き戻しに任せると、`alive` と
            // `dead_tx` のどちらが先に落ちるかは決まっていない。`alive` が
            // 先ならウィンドウ側が抜けて `try_recv` が空を見るので、
            // 「まだ生きている」= 成功に読めてしまう。
            let played = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                play(opts, Box::new(io::screen::Remote(tx)), control)
            }));
            let failed = match played {
                Ok(Ok(())) => false,
                Ok(Err(e)) => {
                    eprintln!("エラー: {e:#}");
                    true
                }
                // パニックの中身は既定のフックがもう出している。
                Err(_) => true,
            };
            let _ = dead_tx.send(failed);
            // ここで最後の送信側が落ちるので、ウィンドウ側のループも抜ける。
            drop(alive);
        });
        // ウィンドウを閉じたらプロセスごと終わる。
        let closed = io::screen::run(rx, press_tx);
        if let Err(e) = &closed {
            eprintln!("エラー: {e:#}");
        }
        quit(exit_code(closed.is_ok(), dead_rx.try_recv()))
    }
    #[cfg(not(feature = "window"))]
    unreachable!("terminal で分岐済み")
}

/// 終了コードを決める。**ウィンドウとゲーム、どちらの失敗も拾う。**
///
/// ゲーム側は別スレッドなので、`main` に見えるのは口の状態だけになる。
/// `unwrap_or(false)` で済ませていた頃は、**結果を返さずに死んだ場合**、
/// つまりパニックが成功に見えていた。口が閉じているのに何も来ていない、
/// が「死んだ」の合図。
#[cfg(feature = "window")]
fn exit_code(window_ok: bool, game: Result<bool, std::sync::mpsc::TryRecvError>) -> i32 {
    use std::sync::mpsc::TryRecvError;
    let game_ok = match game {
        // 結果を返して終わった。
        Ok(failed) => !failed,
        // まだ生きている = ウィンドウを先に閉じた。遊べていたので成功。
        Err(TryRecvError::Empty) => true,
        // 結果を返さずに口が閉じた = パニックで死んだ。
        Err(TryRecvError::Disconnected) => false,
    };
    i32::from(!(window_ok && game_ok))
}

/// 後片付けをせずにプロセスを終わらせる。
///
/// ウィンドウを閉じた時点でも、ゲーム側のスレッドは音を鳴らしていたり
/// whisper で認識している最中だったりする。止める手立ては無いので、
/// そのまま main を返す（= `exit(3)` を通る）ことになるが、これだと
/// whisper.cpp の Metal バックエンドが atexit で GPU デバイスを畳む際に
/// 「まだ誰かがバッファを握っている」と気づいて落ちる。
///
/// ```text
/// ggml-metal-device.m: GGML_ASSERT([rsets->data count] == 0) failed
/// ```
///
/// 終わるだけなので解放し損ねても困らない。`_exit(2)` で C++ の静的
/// デストラクタを走らせずに抜ける。バッファリングされた出力は
/// 捨てられてしまうので、先に流しておく。
#[cfg(feature = "window")]
fn quit(code: i32) -> ! {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    extern "C" {
        fn _exit(code: i32) -> !;
    }
    unsafe { _exit(code) }
}

fn play(opts: Options, screen: Box<dyn Screen>, control: Box<dyn Control>) -> Result<()> {
    let dev = Devices {
        player: Box::new(io::player::Speakers::new()?),
        listener: open_listener(&opts)?,
        screen,
        control,
        camera: open_camera(&opts)?,
    };
    menu::Home::new(catalog(&opts.config), dev, opts.once).run(opts.start)
}

/// **カメラが無ければ起動時に止める。** 選んでから気づくと、テレビの前で
/// 遊びが始まらない。開くだけで、撮るのは遊びが最初に見たとき。
#[cfg(feature = "camera")]
fn open_camera(opts: &Options) -> Result<Box<dyn kit::Camera>> {
    Ok(Box::new(io::camera::Webcam::open(&opts.config)?))
}

/// カメラ無しで組んだときは、何も映らない絵を返す。マウスで押して試す。
#[cfg(not(feature = "camera"))]
fn open_camera(_opts: &Options) -> Result<Box<dyn kit::Camera>> {
    Ok(Box::new(io::camera::Still))
}

#[cfg(feature = "whisper")]
fn open_listener(opts: &Options) -> Result<Box<dyn Listener>> {
    if opts.keyboard {
        return Ok(Box::new(io::listener::Keyboard));
    }
    let ears = io::audio::Ears::new(&opts.config)?;
    Ok(Box::new(io::listener::Mic::new(ears, &opts.config)?))
}

#[cfg(not(feature = "whisper"))]
fn open_listener(_opts: &Options) -> Result<Box<dyn Listener>> {
    Ok(Box::new(io::listener::Keyboard))
}

#[cfg(test)]
mod tests {
    use super::{parse_flags, Flags};

    fn parse(args: &[&str]) -> Result<Flags, String> {
        parse_flags(args.iter().map(|s| s.to_string())).map_err(|e| e.to_string())
    }

    /// **指定したのに効いていない、が一番たちが悪い。** 拾い読みしていた頃は
    /// `--config` のタイプミスが黙って既定のマイクでの起動になり、精度が
    /// 出ない理由が分からなかった。
    #[test]
    fn broken_command_lines_are_refused() {
        // タイポ。以前は黙って無視されていた。
        assert!(parse(&["--termnal"]).unwrap_err().contains("--termnal"));
        // 値の無い --config。以前は既定に落ちていた。
        assert!(parse(&["--config"]).unwrap_err().contains("--config"));
        assert!(parse(&["--game"]).unwrap_err().contains("--game"));
        // 位置引数も受け取らない。
        assert!(parse(&["config.toml"]).is_err());
        // 使い方は弾いたときにも出す。探しに行かせない。
        assert!(parse(&["--nope"]).unwrap_err().contains("--config <path>"));
        // 値が次の引数を食べてしまわないこと自体は許す（ファイル名として扱う）。
        // 拾えるのは形だけで、中身の妥当性は config 側が見る。
        assert_eq!(
            parse(&["--config", "--once"]).unwrap().config,
            Some("--once".into())
        );
    }

    #[test]
    fn flags_are_collected() {
        let f = parse(&["--keyboard", "--once", "--config", "other.toml"]).unwrap();
        assert!(f.keyboard && f.once);
        assert_eq!(f.config, Some("other.toml".into()));
        assert_eq!(f.game, None);
        let f = parse(&["--game", "donna-iro"]).unwrap();
        assert_eq!(f.game.as_deref(), Some("donna-iro"));

        // ウィンドウが無いビルドでは、既定でターミナルに落ちる。
        assert_eq!(parse(&[]).unwrap().terminal, cfg!(not(feature = "window")));
        assert!(parse(&["--terminal"]).unwrap().terminal);
        // 訊いただけで失敗にしない。
        assert!(parse(&["--help"]).unwrap().help);
        assert!(parse(&["-h"]).unwrap().help);
    }
}

#[cfg(all(test, feature = "window"))]
mod exit_tests {
    use std::sync::mpsc::TryRecvError::{Disconnected, Empty};

    use super::exit_code;

    /// **ここが0を返すと障害が成功に見える。** テレビに繋ぎっぱなしの玩具は
    /// systemd の再起動条件しか自分を直す手立てが無いので、終了コードを
    /// 間違えると音の出ないウィンドウが一日中残る。
    #[test]
    fn failures_on_either_side_are_reported() {
        // 何事もなく遊び終えた。
        assert_eq!(exit_code(true, Ok(false)), 0);
        // ウィンドウを先に閉じた。ゲーム側はまだ動いている。
        assert_eq!(exit_code(true, Err(Empty)), 0);

        // ゲーム側が失敗を返した（音源やデバイスの障害）。
        assert_eq!(exit_code(true, Ok(true)), 1);
        // ウィンドウ側が失敗した。
        assert_eq!(exit_code(false, Ok(false)), 1);
        // **結果を返さずに口が閉じた = パニック。**
        // ここが `unwrap_or(false)` で0になっていた。
        assert_eq!(exit_code(true, Err(Disconnected)), 1);
    }
}
