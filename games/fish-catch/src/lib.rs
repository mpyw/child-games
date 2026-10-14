//! おさかなキャッチ！
//!
//! カメラに映った自分の姿の下半分が池になり、魚やフグ、タコ、サメが泳いで
//! いる。魚の所に手を入れるとつかめて、勢いよく振り上げると釣れる。振り上げが
//! 弱いと、スルッと逃げられる。曲が終わったら、釣った数を出し、釣ったものを
//! 並べてお祝いする。
//!
//! **装置にもファイルにも触らない。** 見る・鳴らす・映す・押されたは
//! `kit` のトレイト越しにだけ受け取る。動きの向きと速さは `vision` が出し、
//! 魚がどうなるかは見えたものと時刻を受け取る純粋な計算（`pond.rs`）なので、
//! 装置なしでテストできる。

mod config;
mod cue;
mod look;
mod play;
mod pond;

pub use config::Config;
pub use play::FishCatch;

/// この遊びの名前。`--game` で指し、素材は `games/<ID>/assets/` に置く。
pub const ID: &str = "fish-catch";

/// 鳴らしうる素材の名前。
///
/// `kit::Game::sounds` と同じものだが、**`home` の埋め込みの表は const で
/// 突き合わせる**ので、トレイト越しではなくこちらを読む。
pub const SOUNDS: [&str; cue::Cue::COUNT] = cue::Cue::stems();
