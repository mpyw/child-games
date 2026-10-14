#!/usr/bin/env python3
"""ふうせんタッチの音を合成する。

曲・割れた音・お祝いを、すべて数式から作る。録音も既存の素材も使わない
ので、権利の心配が無く、そのままコミットできる。

    uv run --with numpy python3 games/balloon-crash/tools/make_sounds.py

出力は games/balloon-crash/assets/*.wav（22.05kHz・モノラル・16bit）。
乱数の種を固定してあるので、何度作っても同じものになる。
"""

import os
import wave

import numpy as np

OUT = "games/balloon-crash/assets"
SR = 22050
BPM = 120
BEAT = 60.0 / BPM
BAR = BEAT * 4

RNG = np.random.default_rng(20261014)

# ---------------------------------------------------------------- 音色


def midi(n):
    return 440.0 * 2 ** ((n - 69) / 12.0)


def envelope(n, attack, decay):
    """立ち上がり attack 秒、そのあと decay 秒で 1/e に減衰する。"""
    t = np.arange(n) / SR
    env = np.exp(-np.maximum(t - attack, 0) / decay)
    a = max(1, int(attack * SR))
    env[:a] *= np.linspace(0, 1, a)
    # 末尾で切れてプチッと鳴らないよう、最後の 10ms を落とす。
    r = min(n, int(0.01 * SR))
    env[-r:] *= np.linspace(1, 0, r)
    return env


def marimba(note, dur):
    """木琴に寄せた音。基音と、少しの倍音。すぐ減衰する。"""
    n = int((dur + 0.5) * SR)
    t = np.arange(n) / SR
    f = midi(note)
    wave_ = (np.sin(2 * np.pi * f * t)
             + 0.25 * np.sin(2 * np.pi * 4 * f * t) * np.exp(-t / 0.05)
             + 0.10 * np.sin(2 * np.pi * 10 * f * t) * np.exp(-t / 0.02))
    return wave_ * envelope(n, 0.004, 0.35)


def glock(note, dur):
    """鉄琴。高く、長く響く。"""
    n = int((dur + 1.0) * SR)
    t = np.arange(n) / SR
    f = midi(note)
    wave_ = np.sin(2 * np.pi * f * t) + 0.3 * np.sin(2 * np.pi * 2.76 * f * t) * np.exp(-t / 0.1)
    return wave_ * envelope(n, 0.002, 0.6)


def bass(note, dur):
    n = int(dur * SR)
    t = np.arange(n) / SR
    f = midi(note)
    wave_ = np.sin(2 * np.pi * f * t) + 0.3 * np.sin(2 * np.pi * 2 * f * t)
    return wave_ * envelope(n, 0.01, 0.4)


def pluck(note, dur):
    """和音を刻む、はじく音。"""
    n = int(dur * SR)
    t = np.arange(n) / SR
    f = midi(note)
    wave_ = np.sin(2 * np.pi * f * t) + 0.4 * np.sin(2 * np.pi * 2 * f * t) * np.exp(-t / 0.08)
    return wave_ * envelope(n, 0.003, 0.15)


def brass(note, dur):
    """ファンファーレのラッパに寄せた音。倍音を重ねて、少し揺らす。"""
    n = int((dur + 0.15) * SR)
    t = np.arange(n) / SR
    f = midi(note) * (1 + 0.004 * np.sin(2 * np.pi * 5.5 * t) * np.minimum(t / 0.3, 1))
    phase = 2 * np.pi * np.cumsum(f) / SR
    wave_ = sum(np.sin(k * phase) / k for k in range(1, 7))
    env = np.minimum(np.arange(n) / (0.02 * SR), 1.0)
    rel = int(0.15 * SR)
    env[-rel:] *= np.linspace(1, 0, rel)
    return wave_ * env * 0.5


def kick():
    n = int(0.18 * SR)
    t = np.arange(n) / SR
    f = 50 + 90 * np.exp(-t / 0.03)
    return np.sin(2 * np.pi * np.cumsum(f) / SR) * envelope(n, 0.001, 0.06)


def shaker():
    n = int(0.06 * SR)
    noise = RNG.standard_normal(n)
    # 一次の差分で高い音に寄せる。シャカシャカに聞こえる。
    noise = np.diff(noise, prepend=0)
    return noise * envelope(n, 0.005, 0.015)


def cymbal(dur):
    n = int(dur * SR)
    noise = np.diff(RNG.standard_normal(n), prepend=0)
    return noise * envelope(n, 0.002, dur / 3)


# ---------------------------------------------------------------- 譜面

# 和音。根音と三和音（4オクターブ目）。
CHORDS = {
    "C": (48, [60, 64, 67]),
    "G": (43, [59, 62, 67]),
    "Am": (45, [60, 64, 69]),
    "F": (41, [60, 65, 69]),
    "Em": (40, [59, 64, 67]),
}

N = {name: v for name, v in zip(
    ["G4", "A4", "B4", "C5", "D5", "E5", "F5", "G5", "A5", "B5", "C6", "D6"],
    [67, 69, 71, 72, 74, 76, 77, 79, 81, 83, 84, 86])}
R = None  # 休符


def notes(s):
    """"E5 .5 G5 1 R 2" のような並びを (音, 拍) にする。"""
    toks = s.split()
    return [(None if a == "R" else N[a], float(b)) for a, b in zip(toks[::2], toks[1::2])]


INTRO = (["C", "G"], notes("R 4  R 2 G4 .5 A4 .5 B4 .5 D5 .5"))

A = (["C", "G", "Am", "F", "C", "F", "G", "C"], notes("""
    E5 .5 G5 .5 G5 1 E5 .5 C5 .5 D5 1
    D5 .5 G5 .5 G5 1 B4 .5 D5 .5 G4 1
    C5 .5 E5 .5 A5 1 G5 .5 E5 .5 C5 1
    F5 .5 E5 .5 D5 .5 C5 .5 A4 2
    E5 .5 G5 .5 C6 1 B5 .5 A5 .5 G5 1
    A5 .5 G5 .5 F5 .5 E5 .5 F5 1 A5 1
    G5 .5 F5 .5 E5 .5 D5 .5 B4 1 D5 1
    C5 1 E5 .5 G5 .5 C6 2
"""))

B = (["F", "G", "Em", "Am", "F", "G", "C", "C"], notes("""
    A5 1 A5 .5 G5 .5 F5 1 C5 1
    B5 1 B5 .5 A5 .5 G5 1 D5 1
    G5 .5 E5 .5 B4 .5 E5 .5 G5 1 B5 1
    A5 1.5 G5 .5 E5 1 C5 1
    F5 .5 F5 .5 A5 .5 C6 .5 A5 1 F5 1
    G5 .5 G5 .5 B5 .5 D6 .5 B5 1 G5 1
    C6 .5 B5 .5 A5 .5 G5 .5 E5 .5 D5 .5 C5 1
    E5 1 R 1 G4 .5 A4 .5 B4 .5 D5 .5
"""))

OUTRO = (["C", "F", "G", "C"], notes("""
    C6 1 G5 1 E5 1 G5 1
    A5 1 F5 1 C5 1 A5 1
    B5 .5 A5 .5 G5 .5 F5 .5 D5 1 B4 1
    C5 .5 E5 .5 G5 .5 C6 2.5
"""))


def song():
    # 序奏 2 + A 8 + B 8 + A 8 + 終わり 4 = 30小節 = 60秒。
    sections = [
        (INTRO, dict(drums=False, sparkle=False)),
        (A, dict(drums=True, sparkle=False)),
        (B, dict(drums=True, sparkle=False)),
        (A, dict(drums=True, sparkle=True)),
        (OUTRO, dict(drums=True, sparkle=True)),
    ]
    bars = sum(len(s[0][0]) for s in sections)
    total = bars * BAR + 1.0
    out = np.zeros(int(total * SR) + SR)

    def put(sig, at, gain):
        i = int(at * SR)
        out[i:i + len(sig)] += sig[: len(out) - i] * gain

    bar0 = 0
    for (chords, melody), opt in sections:
        last = bar0 + len(chords) - 1
        for k, name in enumerate(chords):
            root, triad = CHORDS[name]
            t0 = (bar0 + k) * BAR
            final = bar0 + k == last and (chords, melody) == OUTRO
            if final:
                # 最後は和音をのばして終わる。
                put(bass(root, BAR), t0, 0.35)
                for nt in triad:
                    put(marimba(nt, BAR) * envelope(len(marimba(nt, BAR)), 0.004, 1.2), t0, 0.12)
                put(kick(), t0, 0.4)
                put(cymbal(2.0), t0, 0.05)
                continue
            # ブン・チャッ。1拍目と3拍目に根音、2拍目と4拍目に和音。
            put(bass(root, BEAT), t0, 0.35)
            put(bass(root + 7 if name != "Em" else root + 7, BEAT), t0 + 2 * BEAT, 0.3)
            for b in (1, 3):
                for nt in triad:
                    put(pluck(nt, BEAT * 0.8), t0 + b * BEAT, 0.09)
            if opt["drums"]:
                for b in (0, 2):
                    put(kick(), t0 + b * BEAT, 0.4)
                for e in range(8):
                    put(shaker(), t0 + e * BEAT / 2, 0.05 if e % 2 else 0.08)
        t = bar0 * BAR
        for nt, beats in melody:
            dur = beats * BEAT
            if nt is not None:
                put(marimba(nt, dur), t, 0.32)
                if opt["sparkle"]:
                    put(glock(nt + 12, dur), t, 0.07)
            t += dur
        bar0 += len(chords)

    out = out[: int(total * SR)]
    return out / np.max(np.abs(out)) * 0.6


def pop(f0, chime):
    """割れた音。パンと弾ける雑音と、下がる音、最後に小さな鈴。"""
    n = int(0.45 * SR)
    t = np.arange(n) / SR
    burst = np.diff(RNG.standard_normal(n), prepend=0) * envelope(n, 0.0005, 0.012)
    f = f0 * (0.45 + 0.55 * np.exp(-t / 0.05))
    bloop = np.sin(2 * np.pi * np.cumsum(f) / SR) * envelope(n, 0.001, 0.06)
    bell = np.zeros(n)
    i = int(0.05 * SR)
    g = glock(chime, 0.1)[: n - i]
    bell[i:i + len(g)] = g
    out = 0.7 * burst + 0.8 * bloop + 0.35 * bell
    return out / np.max(np.abs(out)) * 0.9


def fanfare():
    """たたたたーん・たたーん。最後は和音をのばす。"""
    seq = [("G4", .25), ("C5", .25), ("E5", .25), ("G5", .75), ("E5", .25), ("G5", 1.5)]
    total = sum(b for _, b in seq) * BEAT + 2.6
    out = np.zeros(int(total * SR) + SR)
    t = 0.0
    for name, beats in seq:
        sig = brass(N[name], beats * BEAT * 0.92)
        i = int(t * SR)
        out[i:i + len(sig)] += sig * 0.35
        t += beats * BEAT
    # 最後の和音。
    i = int(t * SR)
    for nt in (72, 76, 79, 84):
        sig = brass(nt, 2.0)
        out[i:i + len(sig)] += sig * 0.18
    for nt in (84, 88, 91, 96):
        sig = glock(nt, 0.3)
        out[i:i + len(sig)] += sig * 0.08
    cy = cymbal(2.4)
    out[i:i + len(cy)] += cy * 0.12
    k = kick()
    out[i:i + len(k)] += k * 0.5
    out = out[: int(total * SR)]
    return out / np.max(np.abs(out)) * 0.85


def write(name, sig):
    path = os.path.join(OUT, name + ".wav")
    pcm = np.clip(sig, -1, 1)
    pcm = (pcm * 32767).astype("<i2")
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())
    print(f"{path}  {len(sig) / SR:.2f}秒")


def main():
    os.makedirs(OUT, exist_ok=True)
    write("song", song())
    # 割れた音は3通り。下がり方の高さと鈴の音をずらす。
    for i, (f0, chime) in enumerate([(900, 88), (1100, 91), (1300, 96)], start=1):
        write(f"pop-{i}", pop(f0, chime))
    write("fanfare", fanfare())


if __name__ == "__main__":
    main()
