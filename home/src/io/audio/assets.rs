//! 素材の在処。ファイルから読むか、バイナリに埋め込んだものを読むか。
//!
//! **素材は遊びごとに `games/<id>/assets/` に置く。** 中身がフリーの遊びは
//! そのままリポジトリに入れ、そうでないもの（どんないろがすき の歌は
//! つくよみちゃんの音源で、公開できない）は private のサブモジュールにする。
//! どちらでも在処は同じなので、ここからは区別が見えない。

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
#[cfg(feature = "embed-audio")]
use const_for::const_for;
use kit::Sound;

use super::Clip;
use crate::config::Config;

/// ファイルから読むか、バイナリに埋め込んだものを読むか。
/// `rodio::Decoder` に渡すため、どちらも同じ型にまとめる。
pub enum Media {
    File(BufReader<File>),
    #[cfg(feature = "embed-audio")]
    Memory(std::io::Cursor<&'static [u8]>),
}

impl Media {
    pub fn open(sound: Sound) -> Result<Self> {
        let Sound { game, name } = sound;

        // 埋め込みがあり、かつディレクトリ指定で上書きされていなければ
        // バイナリの中から鳴らす。
        #[cfg(feature = "embed-audio")]
        if root_override().is_none() {
            let bytes = embedded(sound)
                .with_context(|| format!("埋め込まれていない素材: {game}/{name}"))?;
            return Ok(Media::Memory(std::io::Cursor::new(bytes)));
        }

        let path = dir_of(game)?.join(format!("{name}.wav"));
        let file = File::open(&path).with_context(|| format!("音源がない: {}", path.display()))?;
        Ok(Media::File(BufReader::new(file)))
    }
}

impl Read for Media {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Media::File(f) => f.read(buf),
            #[cfg(feature = "embed-audio")]
            Media::Memory(c) => c.read(buf),
        }
    }
}

impl Seek for Media {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            Media::File(f) => f.seek(pos),
            #[cfg(feature = "embed-audio")]
            Media::Memory(c) => c.seek(pos),
        }
    }
}

/// 遊びを並べた根の指定（`paths.games`）。設定で明示されていればそれを使う。
/// 埋め込みビルドでも、指定があればファイルから読む。
static OVERRIDE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// 遊びごとに決めた在処。**一度決めたら変えない。**
static DIRS: OnceLock<HashMap<&'static str, PathBuf>> = OnceLock::new();

fn root_override() -> Option<PathBuf> {
    OVERRIDE.get().cloned().flatten()
}

/// 遊びと、その素材の名前の一覧。
pub type Catalog<'a> = &'a [(&'static str, &'static [&'static str])];

/// **起動時に一度だけ呼ぶ。** 素材の在処を決めて、揃っているかまで見る。
///
/// 在処を決めただけで先に進むと、足りないことに気づくのが遊んでいる最中に
/// なる。決めることと確かめることは離しても意味が無いので、ひとつにまとめる。
///
/// **メニューに並ぶ遊びは全部見る。** 選ばれてから見るのでは、テレビの
/// 前で選んだ瞬間に落ちる。
pub fn configure(cfg: &Config, games: Catalog) -> Result<()> {
    // **黙って捨てない。** 二度目の `set` は失敗するだけで、指定した在処は
    // 効かないまま先へ進む。`--config` で音源のディレクトリを差し替えたのに
    // 前のまま鳴る、という形で出る。
    if OVERRIDE.set(cfg.paths.games()).is_err() {
        anyhow::bail!("素材の在処はもう決まっている（configure は起動時に一度だけ）");
    }
    let root = root_override().unwrap_or_else(|| PathBuf::from("games"));
    let dirs = games
        .iter()
        .map(|&(game, names)| (game, choose(&root, game, names)))
        .collect();
    // 上で OVERRIDE を一度きりにしてあるので、ここに二度は来ない。
    let _ = DIRS.set(dirs);
    check(games)
}

fn dir_of(game: &str) -> Result<&'static Path> {
    DIRS.get()
        .and_then(|d| d.get(game))
        .map(PathBuf::as_path)
        .with_context(|| format!("素材の在処が決まっていない遊び: {game}"))
}

/// 遊び1つぶんの在処を決める。
///
/// 本番音源が揃っているかを見て自動で選ぶ。本番音源を `assets/` に置いた
/// 時点で、何もしなくてもそちらに切り替わる。合成音を既定にベタ書きすると、
/// 本番音源を置いたあとも合成音が鳴り続けることになる。
///
/// `reference/` が無い遊び（素材がフリーで、最初から本物が入っている）は
/// 1段目で決まるので、この仕掛けは見えない。
fn choose(root: &Path, game: &str, names: &[&str]) -> PathBuf {
    let main = root.join(game).join("assets");
    if complete(&main, names) {
        return main;
    }
    let reference = main.join("reference");
    if complete(&reference, names) {
        eprintln!("  本番音源が無いので合成音を使う: {}", reference.display());
        return reference;
    }
    // どちらも欠けている。確認の側に本番側のパスで報告させる。
    main
}

fn complete(dir: &Path, names: &[&str]) -> bool {
    names.iter().all(|n| dir.join(format!("{n}.wav")).exists())
}

/// 音源をバイナリに埋め込む。ラズパイに1ファイル置くだけで動かせる。
///
/// ```text
/// cargo build --release --features embed-audio
/// ```
///
/// `include_bytes!` はコンパイル時にファイルを要求するので、
/// 音源が揃うまではこのフィーチャーを有効にできない。
///
/// モデルのほうは `embed-model` で埋め込む。両方入れると
/// バイナリ1つで完結する。
#[cfg(feature = "embed-audio")]
macro_rules! table {
    ($game:literal, $sub:expr, $($name:literal),* $(,)?) => {
        &[$(($game, $name, include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/../games/", $game, "/assets/", $sub, $name, ".wav"
        )) as &'static [u8])),*]
    };
}

/// 埋め込むディレクトリ。本番音源か、確認用の合成音か。
///
/// 本番音源が無い状態で `.app` を作りたいときのために、`embed-reference` で
/// 確認用の合成音を埋め込める。**実行時のフォールバック（`choose` が
/// `reference/` に落ちる）は埋め込みには効かない。** `include_bytes!`
/// はファイルの有無を見られないので、こちらは手で切り替えるしかない。
/// `reference/` を持たない遊びには関係ない。
#[cfg(all(feature = "embed-audio", not(feature = "embed-reference")))]
macro_rules! reference_or_not {
    () => {
        ""
    };
}
#[cfg(feature = "embed-reference")]
macro_rules! reference_or_not {
    () => {
        "reference/"
    };
}

/// 1つの遊びぶんの表。`(遊び, 名前, 実体)`。
#[cfg(feature = "embed-audio")]
type Table = &'static [(&'static str, &'static str, &'static [u8])];

/// 埋め込んだ実体。**名前は手で並べる。遊びを足したら、ここに1つ足す。**
///
/// `include_bytes!` はリテラルのパスしか受け付けないので、`Cue::stem()` の
/// ように導くことができない。実体が無ければここで落ちるが、素材を足して
/// 表に書き忘れるほうは下の検査で見る。
#[cfg(feature = "embed-audio")]
const FILES: &[Table] = &[
    table!(
        "donna-iro",
        reference_or_not!(),
        "intro",
        "question",
        "tail",
        "tail-lead",
        "bridge",
        "interlude",
        "all",
        "red",
        "blue",
        "yellow",
        "green",
        "yellowgreen",
        "white",
        "black",
        "pink",
        "orange",
        "purple",
        "brown",
        "lightblue",
    ),
    // 素材がフリーで、最初から本物が入っている。合成音の切り替えは無い。
    table!(
        "balloon-crash",
        "",
        "song",
        "pop-1",
        "pop-2",
        "pop-3",
        "fanfare"
    ),
    table!("fish-catch", "", "song", "grab", "catch", "slip", "fanfare"),
];

/// const で使う文字列の等値。`==` は const で回せない。
#[cfg(feature = "embed-audio")]
const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    const_for!(i in 0..a.len() => {
        if a[i] != b[i] {
            return false;
        }
    });
    true
}

/// `game` の素材が全部表に載っているか。
#[cfg(feature = "embed-audio")]
const fn listed(game: &str, names: &[&str]) -> bool {
    const_for!(i in 0..names.len() => {
        let mut found = false;
        const_for!(t in 0..FILES.len() => {
            const_for!(j in 0..FILES[t].len() => {
                if str_eq(FILES[t][j].0, game) && str_eq(FILES[t][j].1, names[i]) {
                    found = true;
                }
            });
        });
        if !found {
            return false;
        }
    });
    true
}

/// **素材がすべて表に載っているか、コンパイル時に確かめる。遊びを足したら
/// ここにも1行足す。**
///
/// 実体が無ければ `include_bytes!` がその場で落ちるので、ここで見るのは
/// 名前の取りこぼしだけ。素材を足して上の表に書き忘れると、以前は鳴らそう
/// とした時点で初めて「埋め込まれていない素材」になっていた。
#[cfg(feature = "embed-audio")]
const _: () = {
    assert!(
        listed(donna_iro::ID, &donna_iro::SOUNDS),
        "埋め込みの表に無い素材がある（assets.rs の FILES を見ること）"
    );
    assert!(
        listed(balloon_crash::ID, &balloon_crash::SOUNDS),
        "埋め込みの表に無い素材がある（assets.rs の FILES を見ること）"
    );
    assert!(
        listed(fish_catch::ID, &fish_catch::SOUNDS),
        "埋め込みの表に無い素材がある（assets.rs の FILES を見ること）"
    );
};

#[cfg(feature = "embed-audio")]
fn embedded(sound: Sound) -> Option<&'static [u8]> {
    FILES
        .iter()
        .flat_map(|t| t.iter())
        .find(|(g, n, _)| *g == sound.game && *n == sound.name)
        .map(|(_, _, b)| *b)
}

/// 素材が揃っているか確かめる。足りないものはまとめて出す。
///
/// 在処が決まっていることが前提なので、`configure` からしか呼ばない。
///
/// **開けるかどうかだけでなく、実際に復号する。** 開けるが壊れている wav は
/// 開くだけの検査を素通りして、鳴らそうとした時点で初めて落ちる。それでは
/// 「遊んでいる最中に落ちない」を保証したことにならない。
fn check(games: Catalog) -> Result<()> {
    // **「無い」と「壊れている」を丸めない。** 置いてあるのに「足りない」と
    // 言われると、探す方向を誤る。理由はそのまま出す。
    let mut bad: Vec<String> = Vec::new();
    let mut total = 0;
    for &(game, names) in games {
        for &name in names {
            total += 1;
            match Clip::load(Sound { game, name }) {
                Ok(clip) if clip.total().is_zero() => {
                    // 切り詰められた wav はヘッダだけ読めて中身が空になる。
                    bad.push(format!("{game}/{name}: 中身が空"));
                }
                Ok(_) => {}
                Err(e) => bad.push(format!("{game}/{name}: {e:#}")),
            }
        }
    }
    if bad.is_empty() {
        return Ok(());
    }
    anyhow::bail!(
        "鳴らせない音源がある（{} / {total}）\n  {}",
        bad.len(),
        bad.join("\n  ")
    )
}
