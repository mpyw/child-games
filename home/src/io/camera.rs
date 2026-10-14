//! カメラ。`kit::Camera` の実装。
//!
//! 遊びに渡すのは**縮めて左右を反転した絵**だけ。何が動いたかを見るのは
//! 遊びの仕事なので、ここは撮って整えるところまで。
//!
//! **映像は保存も送信もしない。** 子どもが映るため。撮った絵は遊びに渡して
//! 画面に出すだけで、どこにも書かない。

mod shrink;
#[cfg(not(feature = "camera"))]
mod still;
#[cfg(feature = "camera")]
mod webcam;

#[cfg_attr(not(feature = "camera"), allow(unused_imports))]
pub use shrink::shrink;
#[cfg(not(feature = "camera"))]
pub use still::Still;
#[cfg(feature = "camera")]
pub use webcam::Webcam;
