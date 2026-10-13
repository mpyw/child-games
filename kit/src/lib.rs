//! 遊びと装置の継ぎ目。**ここには口しか置かない。**
//!
//! 遊び（`games/*`）は、鳴らす・映す・聞く・押されたを、ここのトレイト
//! 越しにしか知らない。実体は `home` にあり、メニューから起動するときに
//! `Devices` にまとめて貸す。
//!
//! **遊びから `home` を読んではいけない。** `home` はメニューに並べる
//! ために遊びを読むので、逆を許すと循環する。依存の向きは Cargo が
//! 見張っているので、うっかり装置に手を伸ばすとビルドが通らない。
//!
//! 唯一の例外は時間。遊びは `Instant::now` と `sleep` を使う。装置や
//! ファイルは触らないので、ここでは純粋として扱う。

mod control;
mod game;
mod listener;
mod player;
mod screen;

// **中の割り方を外に見せない。** 使う側はここだけ見れば繋げられる。
pub use control::{Control, Press};
pub use game::{Devices, Ended, Game};
pub use listener::{Heard, Listener};
pub use player::{Player, Sound, Timing};
pub use screen::{Rect, Rgb, Scene, Screen, Shape, BACKGROUND, HEIGHT, WIDTH};
