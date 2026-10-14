//! スピーカーから鳴らす。rodio を使う。

use anyhow::{Context, Result};
use kit::{Player, Sound, Timing};

use crate::io::audio::Clip;

/// 出力ストリームを持ち回す。再生のたびに開き直すと
/// デバイスの初期化で無視できない間が空く。
pub struct Speakers {
    _stream: rodio::OutputStream,
    handle: rodio::OutputStreamHandle,
    /// 今鳴っているもの。**`detach` していた頃は止められなかった。**
    /// 「戻る」で歌を切るために手元に残す。
    ///
    /// `Player` は `&self` しか渡さないので内側で可変にする。`Speakers`
    /// は1つのスレッドの中だけで使う（rodio が `!Send`）ため、
    /// `RefCell` で足りる。
    playing: std::cell::RefCell<Option<rodio::Sink>>,
    /// 今鳴っている効果音。**口は曲とこれの2本だけ。** 効果音どうしは
    /// 置き換えるので、立て続けに鳴らしても重なり過ぎない。
    effect: std::cell::RefCell<Option<rodio::Sink>>,
}

impl Speakers {
    pub fn new() -> Result<Self> {
        let (_stream, handle) =
            rodio::OutputStream::try_default().context("出力デバイスを開けない")?;
        Ok(Self {
            _stream,
            handle,
            playing: std::cell::RefCell::new(None),
            effect: std::cell::RefCell::new(None),
        })
    }
}

impl Player for Speakers {
    /// **鳴らし始めて長さだけ返す。待たない。**
    ///
    /// 待つのは遊びの側。そうしないと待っている間に「やめる」を
    /// 見られない（`kit::Player` を見ること）。
    ///
    /// 末尾の無音の長さは素材から測る。決め打ちにすると、つくよみちゃんの
    /// 音源に差し替えたときに合わなくなる。
    fn begin(&self, sound: Sound) -> Result<Timing> {
        let clip = Clip::load(sound)?;
        let timing = Timing {
            total: clip.total(),
            audible: clip.audible(),
        };
        let sink = rodio::Sink::try_new(&self.handle)?;
        sink.append(clip.into_source());
        // **前のものを置き換える。** ここで落ちる古い Sink は drop で
        // 止まるので、鳴り残りが重なることはない。
        *self.playing.borrow_mut() = Some(sink);
        Ok(timing)
    }

    /// 曲を止めずに重ねる。前の効果音は置き換える。
    fn effect(&self, sound: Sound) -> Result<()> {
        let clip = Clip::load(sound)?;
        let sink = rodio::Sink::try_new(&self.handle)?;
        sink.append(clip.into_source());
        *self.effect.borrow_mut() = Some(sink);
        Ok(())
    }

    /// 鳴っているものを止める。曲も効果音も。**何も鳴っていなければ
    /// 何もしない。**
    fn silence(&self) {
        // 手放すだけで止まる（`Sink` は drop で止める）が、`stop` を
        // 呼んでおくほうが意図がはっきりする。
        for slot in [&self.playing, &self.effect] {
            if let Some(sink) = slot.borrow_mut().take() {
                sink.stop();
            }
        }
    }
}
