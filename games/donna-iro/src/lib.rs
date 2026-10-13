//! どんないろがすき？
//!
//! 「どんな色がすき？」と呼びかけ、子どもが叫んだ色名を聞き取って
//! その色の続きを再生する。進行は `play.rs` の頭に図がある。
//!
//! **装置にもファイルにも触らない。** 鳴らす・映す・聞く・押されたは
//! `kit` のトレイト越しにだけ受け取る。そのおかげで、進行はまるごと
//! 装置なしでテストできる（`play.rs`）。

mod color;
mod config;
mod cue;
mod look;
mod matcher;
mod play;

pub use config::Config;
pub use play::DonnaIro;

/// この遊びの名前。`--game` で指し、素材は `games/<ID>/assets/` に置く。
pub const ID: &str = "donna-iro";

/// 鳴らしうる素材の名前。
///
/// `kit::Game::sounds` と同じものだが、**`home` の埋め込みの表は const で
/// 突き合わせる**ので、トレイト越しではなくこちらを読む。
pub const SOUNDS: [&str; cue::Cue::COUNT] = cue::Cue::stems();
