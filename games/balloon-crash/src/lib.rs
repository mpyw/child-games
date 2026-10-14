//! ふうせんタッチ
//!
//! カメラに映った自分の姿の上に風船が浮かぶ。手で触ると割れる。曲が
//! 終わったら、割った数を出し、割った風船を並べてお祝いする。
//!
//! **装置にもファイルにも触らない。** 見る・鳴らす・映す・押されたは
//! `kit` のトレイト越しにだけ受け取る。動いたかどうかの判断も、風船の
//! 当たり判定も、絵と時刻を受け取る純粋な計算なので、装置なしで
//! テストできる（`motion.rs`、`field.rs`）。

mod config;
mod cue;
mod field;
mod look;
mod motion;
mod play;

pub use config::Config;
pub use play::BalloonCrash;

/// この遊びの名前。`--game` で指し、素材は `games/<ID>/assets/` に置く。
pub const ID: &str = "balloon-crash";

/// 鳴らしうる素材の名前。
///
/// `kit::Game::sounds` と同じものだが、**`home` の埋め込みの表は const で
/// 突き合わせる**ので、トレイト越しではなくこちらを読む。
pub const SOUNDS: [&str; cue::Cue::COUNT] = cue::Cue::stems();
