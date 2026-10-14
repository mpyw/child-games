# fonts

`ui.ttf` は **Noto Sans JP** から、画面に出す字だけを抜いたサブセット。

ウィンドウに出す字は「もう1回」のボタン、メニューに並ぶ遊びの題、遊びが画面に出す字（点数など）だけなので、
全部入りの CJK フォント（5MB 超）を抱える理由がない。三十字ほどに削ると 16KB で済む。

作り直すには（`fonttools` が要る。`uvx --from fonttools fonttools …` でもよい）:

```sh
curl -sL -o vf.ttf \
  https://raw.githubusercontent.com/notofonts/noto-cjk/main/Sans/Variable/TTF/Subset/NotoSansJP-VF.ttf
python3 -m fontTools.varLib.instancer vf.ttf wght=700 -o bold.ttf
python3 -m fontTools.subset bold.ttf --text='もう1回どんないろがすき？ふうせんタッチ0123456789こ わったね！' --name-IDs='*' \
  --no-hinting --desubroutinize --output-file=ui.ttf
```

可変フォントのままだと `fontdue` が扱えないので、先に太さを固定する。
太字にしてあるのは、テレビから離れて読むため。

> [!IMPORTANT]
> **遊びを足したら、その題の字と `Game::glyphs` の字を `--text` に足してサブセットし直すこと。**
> 入っていない字は何も描かれない。足りなければ
> `every_label_on_screen_is_in_the_font`（`src/io/screen/window.rs`）が落ちる。

ライセンスは SIL Open Font License 1.1（`LICENSE-NotoSansJP.txt`）。
サブセットしても OFL のままなので、この表示ごと残すこと。
