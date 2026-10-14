//! 外の世界に触るもの。`kit` のトレイトの実装が並ぶ。
//!
//! スピーカー、マイク、カメラ、画面、標準入出力、リモコン。どれも `kit` の
//! トレイトを実装するか、`kit` の型を組み立てて返すだけで、遊びの進行も
//! メニューの進行も知らない。
//!
//! **`io` から `menu` を読んではいけない。** メニューは装置なしで
//! 組み立てられること（`menu` のテストがそれをやっている）。

pub mod audio;
pub mod camera;
pub mod control;
pub mod listener;
pub mod player;
pub mod screen;
