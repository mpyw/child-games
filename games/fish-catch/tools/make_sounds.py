#!/usr/bin/env python3
"""おさかなキャッチ！の音を合成する。

曲・つかんだ音・釣れた音・逃げた音・お祝いを、すべて数式から作る。録音も
既存の素材も使わないので、権利の心配が無く、そのままコミットできる。

    uv run --with numpy python3 games/fish-catch/tools/make_sounds.py

出力は games/fish-catch/assets/*.wav（22.05kHz・モノラル・16bit）。
乱数の種を固定してあるので、何度作っても同じものになる。
"""

import os
import wave

import numpy as np

OUT = "games/fish-catch/assets"
SR = 22050
BPM = 100
BEAT = 60.0 / BPM
BAR = BEAT * 4

RNG = np.random.default_rng(20261015)

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
    """木琴。基音と少しの倍音。すぐ減衰する。"""
    n = int((dur + 0.6) * SR)
    t = np.arange(n) / SR
    f = midi(note)
    w = (np.sin(2 * np.pi * f * t)
         + 0.2 * np.sin(2 * np.pi * 4 * f * t) * np.exp(-t / 0.05))
    return w * envelope(n, 0.004, 0.45)


def vibes(note, dur):
    """ビブラフォン。ゆっくり揺れながら長く響く。水の中のような和音に使う。"""
    n = int((dur + 0.8) * SR)
    t = np.arange(n) / SR
    f = midi(note)
    trem = 1 + 0.25 * np.sin(2 * np.pi * 5.0 * t)
    w = (np.sin(2 * np.pi * f * t) + 0.15 * np.sin(2 * np.pi * 3 * f * t)) * trem
    return w * envelope(n, 0.02, 1.2)


def bass(note, dur):
    n = int(dur * SR)
    t = np.arange(n) / SR
    f = midi(note)
    w = np.sin(2 * np.pi * f * t) + 0.25 * np.sin(2 * np.pi * 2 * f * t)
    return w * envelope(n, 0.01, 0.5)


def bubble(f0):
    """泡がぽこっと上がる音。高さが上へ滑る短い音。"""
    n = int(0.09 * SR)
    t = np.arange(n) / SR
    f = f0 * (1 + 1.5 * t / t[-1])
    return np.sin(2 * np.pi * np.cumsum(f) / SR) * envelope(n, 0.002, 0.025)


def shaker():
    n = int(0.05 * SR)
    noise = np.diff(RNG.standard_normal(n), prepend=0)
    return noise * envelope(n, 0.004, 0.012)


def splash_noise(dur, decay):
    """水しぶき。低めに寄せた雑音。"""
    n = int(dur * SR)
    noise = RNG.standard_normal(n)
    # 二度ならして高い音を落とす。ザーッという水の音に寄る。
    k = np.ones(6) / 6
    noise = np.convolve(np.convolve(noise, k, "same"), k, "same")
    return noise / np.max(np.abs(noise)) * envelope(n, 0.004, decay)


def glock(note, dur):
    n = int((dur + 0.8) * SR)
    t = np.arange(n) / SR
    f = midi(note)
    w = np.sin(2 * np.pi * f * t) + 0.3 * np.sin(2 * np.pi * 2.76 * f * t) * np.exp(-t / 0.1)
    return w * envelope(n, 0.002, 0.5)


def brass(note, dur):
    """ファンファーレのラッパに寄せた音。倍音を重ねて、少し揺らす。"""
    n = int((dur + 0.15) * SR)
    t = np.arange(n) / SR
    f = midi(note) * (1 + 0.004 * np.sin(2 * np.pi * 5.5 * t) * np.minimum(t / 0.3, 1))
    phase = 2 * np.pi * np.cumsum(f) / SR
    w = sum(np.sin(k * phase) / k for k in range(1, 7))
    env = np.minimum(np.arange(n) / (0.02 * SR), 1.0)
    rel = int(0.15 * SR)
    env[-rel:] *= np.linspace(1, 0, rel)
    return w * env * 0.5


# ---------------------------------------------------------------- 譜面

# 和音。根音と三和音。
CHORDS = {
    "F": (41, [60, 65, 69]),
    "Dm": (38, [62, 65, 69]),
    "Bb": (46, [62, 65, 70]),
    "C": (48, [60, 64, 67]),
    "Gm": (43, [62, 67, 70]),
    "Am": (45, [60, 64, 69]),
}

N = {name: v for name, v in zip(
    ["C5", "D5", "E5", "F5", "G5", "A5", "Bb5", "C6", "D6"],
    [72, 74, 76, 77, 79, 81, 82, 84, 86])}


def notes(s):
    """"C5 .5 F5 1 R 2" のような並びを (音, 拍) にする。"""
    toks = s.split()
    return [(None if a == "R" else N[a], float(b)) for a, b in zip(toks[::2], toks[1::2])]


# 1小節に2つ和音があるときは "Gm/C" と書く（2拍ずつ）。
INTRO = (["F"], notes("R 3 C5 1"))

A = (["F", "Dm", "Bb", "C", "F", "Dm", "Gm/C", "F"], notes("""
    C5 .5 F5 .5 A5 1 G5 .5 F5 .5 C5 1
    D5 .5 F5 .5 A5 1 F5 1 D5 1
    D5 .5 F5 .5 Bb5 1 A5 .5 G5 .5 F5 1
    E5 1 G5 1 C6 2
    C6 .5 A5 .5 F5 1 G5 .5 A5 .5 F5 1
    D5 .5 E5 .5 F5 1 A5 1 D6 1
    Bb5 1 A5 .5 G5 .5 E5 1 G5 1
    F5 2 R 1 C5 1
"""))

B = (["Bb", "C", "Am", "Dm", "Gm", "C", "F", "C"], notes("""
    D5 .5 F5 .5 Bb5 1.5 A5 .5 G5 1
    E5 .5 G5 .5 C6 1.5 Bb5 .5 A5 1
    A5 1 G5 .5 E5 .5 C5 1 E5 1
    F5 1.5 E5 .5 D5 2
    G5 .5 A5 .5 Bb5 1 D6 1 Bb5 1
    A5 .5 G5 .5 E5 1 C5 1 E5 1
    F5 1 A5 1 C6 1 A5 1
    G5 2 R 1 C5 1
"""))

# 最後の A は7小節で切り、最後の小節で F をのばして終わる。
A_END = (A[0][:7] + ["F"], A[1][:-3] + notes("F5 4"))


def song():
    # 序奏 1 + A 8 + B 8 + A 8 = 25小節 = 60秒。
    sections = [
        (INTRO, dict(drums=False)),
        (A, dict(drums=False)),
        (B, dict(drums=True)),
        (A_END, dict(drums=True)),
    ]
    bars = sum(len(s[0][0]) for s in sections)
    total = bars * BAR + 1.2
    out = np.zeros(int(total * SR) + SR)

    def put(sig, at, gain):
        i = int(at * SR)
        out[i:i + len(sig)] += sig[: len(out) - i] * gain

    bar0 = 0
    for (chords, melody), opt in sections:
        for k, names in enumerate(chords):
            t0 = (bar0 + k) * BAR
            halves = names.split("/")
            for h, name in enumerate(halves):
                root, triad = CHORDS[name]
                span = BAR / len(halves)
                at = t0 + h * span
                # ゆったりした和音を、小節（または半小節）ごとに置く。
                for nt in triad:
                    put(vibes(nt, span), at, 0.05)
                put(bass(root, BEAT * 1.5), at, 0.32)
                put(bass(root + 7, BEAT), at + span / 2, 0.22)
            if opt["drums"]:
                for e in range(8):
                    put(shaker(), t0 + e * BEAT / 2, 0.04 if e % 2 else 0.06)
            # 泡。小節に2つ、ずらした所でぽこっと上がる。
            for _ in range(2):
                at = t0 + RNG.uniform(0, BAR)
                put(bubble(RNG.uniform(600, 1100)), at, 0.08)
        t = bar0 * BAR
        for nt, beats in melody:
            dur = beats * BEAT
            if nt is not None:
                put(marimba(nt, dur), t, 0.3)
            t += dur
        bar0 += len(chords)

    out = out[: int(total * SR)]
    return out / np.max(np.abs(out)) * 0.6


def grab():
    """ピチピチ。魚が手の中で跳ねる、短い音を3つ。"""
    n = int(0.42 * SR)
    out = np.zeros(n)
    for i, f0 in enumerate([1300, 1500, 1250]):
        m = int(0.07 * SR)
        t = np.arange(m) / SR
        f = f0 * (1 + 0.6 * np.sin(2 * np.pi * 28 * t))
        blip = np.sin(2 * np.pi * np.cumsum(f) / SR) * envelope(m, 0.002, 0.02)
        at = int(i * 0.12 * SR)
        out[at:at + m] += blip
    return out / np.max(np.abs(out)) * 0.7


def catch():
    """ザバーンと水から上がり、きらきらと鈴が上がっていく。"""
    total = 1.4
    out = np.zeros(int(total * SR))
    s = splash_noise(0.7, 0.18)
    out[: len(s)] += s * 0.8
    # 低いドボン。
    n = int(0.25 * SR)
    t = np.arange(n) / SR
    f = 120 + 200 * np.exp(-t / 0.05)
    out[:n] += np.sin(2 * np.pi * np.cumsum(f) / SR) * envelope(n, 0.002, 0.08) * 0.6
    for i, nt in enumerate([84, 88, 91, 96]):
        g = glock(nt, 0.2)
        at = int((0.18 + i * 0.09) * SR)
        g = g[: len(out) - at]
        out[at:at + len(g)] += g * 0.35
    return out / np.max(np.abs(out)) * 0.9


def slip():
    """ツルン。高さが滑り落ちる音と、小さな水音。"""
    n = int(0.6 * SR)
    t = np.arange(n) / SR
    f = 900 * np.exp(-t / 0.18) + 220
    wob = 1 + 0.08 * np.sin(2 * np.pi * 14 * t)
    glide = np.sin(2 * np.pi * np.cumsum(f * wob) / SR) * envelope(n, 0.005, 0.15)
    s = splash_noise(0.3, 0.06)
    out = glide * 0.8
    at = int(0.28 * SR)
    out[at:at + len(s)] += s[: n - at] * 0.35
    return out / np.max(np.abs(out)) * 0.8


def fanfare():
    """たらららったーん。最後は和音をのばす。"""
    seq = [("C5", .25), ("D5", .25), ("E5", .25), ("F5", .25), ("A5", .5), ("C6", 1.5)]
    total = sum(b for _, b in seq) * BEAT + 2.6
    out = np.zeros(int(total * SR) + SR)
    t = 0.0
    for name, beats in seq:
        sig = brass(N[name], beats * BEAT * 0.92)
        i = int(t * SR)
        out[i:i + len(sig)] += sig * 0.35
        t += beats * BEAT
    i = int(t * SR)
    for nt in (65, 69, 72, 77):
        sig = brass(nt, 2.0)
        out[i:i + len(sig)] += sig * 0.18
    for nt in (89, 93, 96, 101):
        sig = glock(nt, 0.3)
        out[i:i + len(sig)] += sig * 0.07
    s = splash_noise(1.2, 0.3)
    out[i:i + len(s)] += s * 0.15
    out = out[: int(total * SR)]
    return out / np.max(np.abs(out)) * 0.85


def write(name, sig):
    path = os.path.join(OUT, name + ".wav")
    pcm = (np.clip(sig, -1, 1) * 32767).astype("<i2")
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())
    print(f"{path}  {len(sig) / SR:.2f}秒")


def main():
    os.makedirs(OUT, exist_ok=True)
    write("song", song())
    write("grab", grab())
    write("catch", catch())
    write("slip", slip())
    write("fanfare", fanfare())


if __name__ == "__main__":
    main()
