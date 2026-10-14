# pi — ラズパイとテレビ

テレビに繋ぎっぱなしのラズパイで動かすための、組み方・配り方・OS 側の設定。

## ハードウェア

| もの | |
| --- | --- |
| Raspberry Pi 5 | **デスクトップ版の OS**（trixie）。コンソール起動だとウィンドウが開けない |
| USB マイク | 離れた場所から拾うなら ReSpeaker などのマイクアレイ HAT |
| USB カメラ | **無いと起動しない。** UVC の Web カメラなら V4L2 で動く。今のマイクの V15AF は Web カメラなので使える見込み（実機では未確認） |
| テレビ | HDMI で繋ぐ |

> [!TIP]
> **リモコンに変換器は要らない。** CEC は HDMI の規格そのものなので、テレビと
> 繋がっていれば `/dev/cec0` に出る。変換器が要るのは PC で試すときだけ。
> 何がどう届くかは `tools/cec-probe.sh` で見る。

## 組んで配る

```sh
tools/build-pi.sh              # 組むだけ → target/pi/donna-iro
tools/build-pi.sh pi5          # 組んで ssh 先の ~/donna-iro へ配る
tools/build-pi.sh pi5 /opt/x   # 配る先を変える
FEATURES=whisper tools/build-pi.sh   # feature を変える
```

配り終えたら:

```sh
ssh pi5 'cd donna-iro && ./donna-iro --terminal --keyboard'
```

### 組み方

**sysroot を使うクロスコンパイルはしない。** macOS から
`aarch64-unknown-linux-gnu` を直接狙うと、sysroot・クロス gcc/g++・libclang の
ターゲット指定が要る。whisper.cpp（cmake + bindgen）と ALSA のためで、
手間の割に静かに壊れる。

代わりに **arm64 の Debian コンテナの中でネイティブに組む。**

| 前提 | 結果 |
| --- | --- |
| Apple Silicon の Mac | linux/arm64 がエミュレーションなしで動く。whisper.cpp 込みで1分を切る |
| Intel Mac | エミュレーションになり、現実的な速さにならない。Pi 側で `cargo build` するほうが早い |

> [!CAUTION]
> **イメージは Pi と同じ trixie に固定してある。新しくしてはいけない。**
> glibc は前方互換でないので、Pi 側で `GLIBC_2.xx not found` になる。

### Pi 側に入れるもの

**Pi 側に rust も cmake も入れない。** 実行時に要るのは次の共有ライブラリだけで、
どれも Raspberry Pi OS のデスクトップ版に既に入っている。足りなければ
スクリプトが名前を出す。

| ライブラリ | 誰が使う |
| --- | --- |
| `libasound.so.2` | cpal / rodio |
| `libstdc++.so.6` | whisper.cpp |
| `libX11.so.6`、`libxkbcommon.so.0` | minifb（`window` のとき） |

### 送るもの

| もの | 置き場所 | |
| --- | --- | --- |
| バイナリ | `~/donna-iro/donna-iro` | |
| `config.toml` | `~/donna-iro/config.toml` | |
| 音源 | `~/donna-iro/games/<name>/assets/` | 遊びごと、手元と同じ並び。本番が無ければ `reference/` を送る |
| whisper のモデル | `~/donna-iro/models/` | `models/*.bin` を全部。`tiny` に落として比べるのもそのままできる |
| OS 側の設定 | 下の表 | |

**埋め込み（`embed`）は使わない。** Pi ではバイナリの隣にファイルを置けるので、
141MB のモデルを焼き込む理由が無い。差し替えも rsync だけで済む。

**音源は手元から送る。** 音源は private サブモジュールなので、Pi に鍵を
置いて `known_hosts` を通すまで、Pi 側では clone できない。


## OS 側の設定

**バイナリに入れられなかったものだけ**がこのディレクトリにある。手で置くと
Git の外に出て、次に組み直したときに再現できない。だからここで持って、
`tools/build-pi.sh` に配らせる。**どれも実機で検証した実物。**

| ここ | 配る先 | 何のため | 読み直し |
| --- | --- | --- | --- |
| `asound.conf` | `/etc/asound.conf` | ssh から効く音量つまみを作る | 次に開いたとき |
| `labwc-rc.xml` | `~/.config/labwc/rc.xml` | ウィンドウを全画面にする | `killall -HUP labwc`（スクリプトが打つ） |
| `kanshi-config` | `~/.config/kanshi/config` | 出力の解像度を 1920×1080 に固定する | kanshi を入れ直す（スクリプトがやる） |

中身が違うものが既にあれば、`*.bak` に退避してから置く。退避したことは必ず言う。

### asound.conf — 音量

**vc4hdmi の `PCM Playback Volume` は効かない。** 値は受け取るが減衰しないので、
`amixer -c 0 sset PCM 5%` は `[-48.40dB]` と表示しても音量が変わらない。
そこで `softvol` を挟んで、実際に効くつまみを作る。

```sh
amixer -c 0 sset Softvol 15%   # 効く
amixer -c 0 sset PCM 15%       # 効かない（触らないこと）
```

`Softvol 0%` は完全な無音なので、音を出せない時間帯でも進行は確かめられる。

> [!IMPORTANT]
> **`/etc/asound.conf` だけは `sudo` が要るので、スクリプトは書き込まない。**
> 違っていれば `/tmp` に置いて、打つべき一行を出す。パスワードを聞かれる
> 経路をスクリプトに隠すと、無人で回したときに黙って止まる。

| 決めごと | 理由 |
| --- | --- |
| **`~/.asoundrc` に置かない** | デスクトップの音量プラグインがあのファイルの所有権を主張する。パネルから出力を切り替えたあとで実際に消えていた。残っているとそちらが勝つ |
| カードは番号ではなく名前（`CARD=vc4hdmi0` / `CARD=V15AF`）で指す | 番号は差し替えでずれる |
| `asym` の `capture.pcm` を書く | 無いと「capture slave is not defined」と `dmix` の警告が大量に出る |

> [!WARNING]
> **マイクを替えたら2箇所を直す。** `asound.conf` の `CARD=V15AF` と、
> `config.toml` の `listen.device`。

**PipeWire は動いているが、当てにできない。** `/etc/alsa/conf.d/` も無いので、
ALSA の `default` は PipeWire を経由しない。cpal もハードを直接叩く。だから
`wpctl` で下げても効かない。

`Softvol` のコントロールは**一度その PCM を使うまで生えない。** 全部ゼロ（= 無音）を
流せば、音を出さずに生やせる。

```sh
aplay -D default -f cd -t raw -d 1 /dev/zero
```

### labwc-rc.xml — 全画面

**minifb にフルスクリーンの API は無い。** `WindowOptions` に該当フィールドが
無く、モニタ解像度を問う口も無い。そこでコンポジタ側で当てる。trixie の
既定セッションは labwc なので、ウィンドウルールで当てる。

```xml
<windowRules>
  <windowRule title="*">
    <action name="ToggleFullscreen"/>
  </windowRule>
</windowRules>
```

**描画側は触らなくてよい。** `resize: true` と `ScaleMode::AspectRatioStretch`
なので、1280×720 のバッファが縦横比を保ったまま画面いっぱいに伸びる。
カーソルはアプリ側で消している（`set_cursor_visibility(false)`）。

**`title="*"` にしているのは、絞れないから。**

- minifb は `get_toplevel()` → `commit()`（= map）→ `set_title` の順に呼ぶ。map の時点でタイトルが空
- `set_app_id` も一度も呼ばない
- labwc は map のときにルールを当てるので、タイトルでも app_id でも一致しない
- X11 backend も `XMapRaised` → `XStoreName` の順で、`XSetClassHint` を呼ばない。`WM_CLASS` すら無い

この Pi は映しっぱなしの玩具なので実害は無い。絞りたければ、minifb に
`set_app_id` か `set_fullscreen` を足した fork を当てる。`wlrctl` を入れて、
起動後にタイトルで当てる手もある。

> [!NOTE]
> **labwc は `-m`（merge-config）で起動しているので、これは
> `/etc/xdg/labwc/rc.xml` の既定に足される。** 素の labwc は最初に見つけた
> 1つしか読まない。`-m` が無い環境では、既定をコピーしてから足すこと。

### kanshi-config — 解像度

**4K に上げさせない。** minifb は 1280×720 のバッファを CPU で拡大する。
3840×2160 だと毎秒2.5億画素になり、ほぼ1コアを食い切る（実測 90%）。
whisper は4コア全部欲しいので、そのぶんが認識時間に乗る。4K で得るのは丸の縁が
滑らかになることだけ。

| 決めごと | 理由 |
| --- | --- |
| リフレッシュレートは書かない | 書くと、その値を持たない表示機に差し替えたとき適用が落ちる |
| 番号ではなくコネクタ名（`HDMI-A-1`）で指す | 同上 |
| `HDMI-A-1` と `HDMI-A-2` の両方にプロファイルを置く | kanshi は繋がっている出力の集合で選ぶ。片方しか書かないと、もう片方のポートで何も当たらない |

kanshi は labwc の autostart で動いている。このビルドには reload が無いので、
直したら入れ直す。`tools/build-pi.sh` が配るときにやる。
