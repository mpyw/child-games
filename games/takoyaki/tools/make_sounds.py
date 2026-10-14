#!/usr/bin/env python3
"""たこやき くるりん！の音を合成する。

曲・効果音・お祝いを、すべて数式から作る。録音も既存の素材も使わないので、
権利の心配が無く、そのままコミットできる。

    uv run --with numpy python3 games/takoyaki/tools/make_sounds.py

出力は games/takoyaki/assets/*.wav（22.05kHz・モノラル・16bit）。
乱数の種を固定してあるので、何度作っても同じものになる。
"""

import os
import wave

import numpy as np

OUT = "games/takoyaki/assets"
SR = 22050
BPM = 120
BEAT = 60.0 / BPM
BAR = BEAT * 4

RNG = np.random.default_rng(20261016)

# ---------------------------------------------------------------- 音色


def midi(n):
    return 440.0 * 2 ** ((n - 69) / 12.0)


def envelope(n, attack, decay):
    """立ち上がり attack 秒、そのあと decay 秒で 1/e に減衰する。"""
    t = np.arange(n) / SR
    env = np.exp(-np.maximum(t - attack, 0) / decay)
    a = max(1, int(attack * SR))
    env[:a] *= np.linspace(0, 1, a)
    r = min(n, int(0.01 * SR))
    env[-r:] *= np.linspace(1, 0, r)
    return env


def fue(note, dur):
    """笛。ビブラートの掛かった澄んだ音に、息の音を少し混ぜる。"""
    n = int((dur + 0.08) * SR)
    t = np.arange(n) / SR
    vib = 1 + 0.006 * np.sin(2 * np.pi * 6.0 * t) * np.minimum(t / 0.15, 1)
    phase = 2 * np.pi * np.cumsum(midi(note) * vib) / SR
    w = np.sin(phase) + 0.12 * np.sin(2 * phase) + 0.05 * np.sin(3 * phase)
    breath = np.diff(RNG.standard_normal(n), prepend=0) * 0.04
    env = np.minimum(t / 0.03, 1.0)
    rel = int(0.08 * SR)
    env[-rel:] *= np.linspace(1, 0, rel)
    return (w + breath) * env


def pluck(note, dur):
    """三味線に寄せた、はじく音。倍音が多く、すぐ減衰する。"""
    n = int(dur * SR)
    t = np.arange(n) / SR
    f = midi(note)
    w = sum(np.sin(2 * np.pi * k * f * t) * np.exp(-t * k * 6) / k for k in range(1, 7))
    return w * envelope(n, 0.002, 0.18)


def bass(note, dur):
    n = int(dur * SR)
    t = np.arange(n) / SR
    f = midi(note)
    w = np.sin(2 * np.pi * f * t) + 0.25 * np.sin(2 * np.pi * 2 * f * t)
    return w * envelope(n, 0.01, 0.35)


def taiko():
    """太鼓のドン。低く、少し下がる。"""
    n = int(0.45 * SR)
    t = np.arange(n) / SR
    f = 55 + 45 * np.exp(-t / 0.05)
    body = np.sin(2 * np.pi * np.cumsum(f) / SR) * envelope(n, 0.002, 0.16)
    hit = np.diff(RNG.standard_normal(n), prepend=0) * envelope(n, 0.0005, 0.008) * 0.4
    return body + hit


def ka():
    """太鼓の縁を打つカッ。"""
    n = int(0.06 * SR)
    t = np.arange(n) / SR
    w = np.sin(2 * np.pi * 1800 * t) * 0.5 + np.diff(RNG.standard_normal(n), prepend=0) * 0.5
    return w * envelope(n, 0.0005, 0.01)


def chappa():
    """チャッパ（小さなシンバル）。金属らしい、揃わない倍音と雑音。"""
    n = int(0.35 * SR)
    t = np.arange(n) / SR
    w = sum(np.sin(2 * np.pi * f * t) for f in (2350, 3170, 4410, 5830)) * 0.2
    noise = np.diff(RNG.standard_normal(n), prepend=0) * 0.5
    return (w + noise) * envelope(n, 0.001, 0.07)


def sizzle(dur, level):
    """ジュージュー。高めの雑音に、ときどきパチッと弾ける音。"""
    n = int(dur * SR)
    noise = RNG.standard_normal(n)
    k = np.ones(3) / 3
    hiss = np.diff(np.convolve(noise, k, "same"), prepend=0)
    # ゆっくり揺らす。一定だと機械の音に聞こえる。
    t = np.arange(n) / SR
    hiss *= 0.7 + 0.3 * np.sin(2 * np.pi * 0.35 * t + RNG.uniform(0, 6))
    out = hiss * level
    for _ in range(int(dur * 6)):
        at = RNG.integers(0, max(1, n - 400))
        m = 300
        out[at:at + m] += np.diff(RNG.standard_normal(m), prepend=0) * envelope(m, 0.0003, 0.002) * level * 3
    return out


def glock(note, dur):
    n = int((dur + 0.8) * SR)
    t = np.arange(n) / SR
    f = midi(note)
    w = np.sin(2 * np.pi * f * t) + 0.3 * np.sin(2 * np.pi * 2.76 * f * t) * np.exp(-t / 0.1)
    return w * envelope(n, 0.002, 0.45)


# ---------------------------------------------------------------- 譜面

CHORDS = {
    "D": (50, [62, 66, 69]),
    "G": (43, [62, 67, 71]),
    "A": (45, [61, 64, 69]),
    "Bm": (47, [62, 66, 71]),
    "F#m": (42, [61, 66, 69]),
}

N = {name: v for name, v in zip(
    ["A4", "B4", "D5", "E5", "F#5", "A5", "B5", "D6"],
    [69, 71, 74, 76, 78, 81, 83, 86])}


def notes(s):
    toks = s.split()
    return [(None if a == "R" else N[a], float(b)) for a, b in zip(toks[::2], toks[1::2])]


INTRO = (["D", "D"], notes("R 4  R 3 A4 1"))

A = (["D", "D", "G", "A", "D", "Bm", "G", "D"], notes("""
    A5 .5 A5 .5 B5 .5 A5 .5 F#5 1 D5 1
    E5 .5 F#5 .5 A5 1 F#5 .5 E5 .5 D5 1
    B4 .5 D5 .5 E5 1 D5 .5 B4 .5 A4 1
    D5 .5 E5 .5 F#5 .5 A5 .5 E5 2
    A5 .5 A5 .5 B5 .5 A5 .5 F#5 1 A5 1
    B5 .5 A5 .5 F#5 1 E5 .5 F#5 .5 D5 1
    E5 1 F#5 .5 E5 .5 D5 .5 B4 .5 A4 1
    D5 2 R 1 A4 1
"""))

B = (["G", "A", "F#m", "Bm", "G", "A", "D", "A"], notes("""
    D6 1 B5 .5 A5 .5 B5 1 D6 1
    A5 1 F#5 .5 E5 .5 F#5 1 A5 1
    F#5 .5 A5 .5 B5 1 A5 .5 F#5 .5 E5 1
    D5 1.5 E5 .5 F#5 2
    B5 .5 D6 .5 B5 .5 A5 .5 B5 1 D6 1
    A5 .5 B5 .5 A5 .5 F#5 .5 E5 2
    D5 .5 E5 .5 F#5 .5 A5 .5 B5 1 A5 1
    A5 2 R 1 A4 1
"""))

OUTRO = (["D", "G", "A", "D"], notes("""
    D6 1 B5 1 A5 1 F#5 1
    E5 1 F#5 1 A5 1 B5 1
    A5 .5 B5 .5 A5 .5 F#5 .5 E5 2
    D5 4
"""))


def song():
    # 序奏 2 + A 8 + B 8 + A 8 + 終わり 4 = 30小節 = 60秒。
    sections = [
        (INTRO, dict(chappa=False)),
        (A, dict(chappa=False)),
        (B, dict(chappa=True)),
        (A, dict(chappa=True)),
        (OUTRO, dict(chappa=True)),
    ]
    bars = sum(len(s[0][0]) for s in sections)
    total = bars * BAR + 1.0
    out = np.zeros(int(total * SR) + SR)

    def put(sig, at, gain):
        i = int(at * SR)
        out[i:i + len(sig)] += sig[: len(out) - i] * gain

    bar0 = 0
    for (chords, melody), opt in sections:
        last_section = (chords, melody) == OUTRO
        for k, name in enumerate(chords):
            root, triad = CHORDS[name]
            t0 = (bar0 + k) * BAR
            final = last_section and k == len(chords) - 1
            put(taiko(), t0, 0.5)
            if final:
                put(bass(root, BAR), t0, 0.3)
                for nt in triad:
                    put(pluck(nt + 12, BAR), t0, 0.08)
                put(chappa(), t0, 0.12)
                continue
            # ドン・カッ・ドン・カカッ。
            put(ka(), t0 + BEAT, 0.12)
            put(taiko(), t0 + 2 * BEAT, 0.4)
            put(ka(), t0 + 3 * BEAT, 0.12)
            put(ka(), t0 + 3.5 * BEAT, 0.08)
            put(bass(root, BEAT), t0, 0.28)
            put(bass(root + 7, BEAT), t0 + 2 * BEAT, 0.22)
            # はじく和音を裏拍に。
            for e in (0.5, 1.5, 2.5, 3.5):
                nt = triad[int(e) % len(triad)]
                put(pluck(nt, BEAT * 0.5), t0 + e * BEAT, 0.07)
            if opt["chappa"]:
                for b in (1, 3):
                    put(chappa(), t0 + b * BEAT, 0.07)
        t = bar0 * BAR
        for nt, beats in melody:
            dur = beats * BEAT
            if nt is not None:
                put(fue(nt, dur * 0.95), t, 0.22)
            t += dur
        bar0 += len(chords)

    # ジュージュー焼ける音を、曲じゅう薄く敷く。
    s = sizzle(total, 0.02)
    out[: len(s)] += s

    out = out[: int(total * SR)]
    return out / np.max(np.abs(out)) * 0.6


def normalized(sig, peak):
    return sig / np.max(np.abs(sig)) * peak


def pour():
    """とろー。なめらかな雑音がふくらんで消える。下がる低い音を添える。"""
    n = int(0.7 * SR)
    t = np.arange(n) / SR
    noise = RNG.standard_normal(n)
    k = np.ones(12) / 12
    smooth = np.convolve(np.convolve(noise, k, "same"), k, "same")
    swell = np.sin(np.pi * t / t[-1]) ** 1.5
    f = 330 - 120 * t / t[-1]
    glug = np.sin(2 * np.pi * np.cumsum(f) / SR) * swell * 0.5
    return normalized(smooth / np.max(np.abs(smooth)) * swell + glug, 0.6)


def poke():
    """じゅっ。

    **山ではなく平均で音量を合わせる。** パチッと弾ける音の山に合わせると、
    ジューの本体が小さくなりすぎて聞こえない。
    """
    sig = sizzle(0.4, 1.0) * envelope(int(0.4 * SR), 0.003, 0.12)
    sig = sig * (0.13 / np.sqrt(np.mean(sig ** 2)))
    return np.clip(sig, -0.95, 0.95)


def ready():
    """ピカッ。鈴が2つ上がる。"""
    out = np.zeros(int(0.8 * SR))
    for i, nt in enumerate([88, 95]):
        g = glock(nt, 0.15)
        at = int(i * 0.09 * SR)
        g = g[: len(out) - at]
        out[at:at + len(g)] += g
    return normalized(out, 0.6)


def flip():
    """くるっ。笛のように上がる音と、小さなぽん。"""
    n = int(0.35 * SR)
    t = np.arange(n) / SR
    f = 600 + 900 * (t / t[-1]) ** 0.7
    w = np.sin(2 * np.pi * np.cumsum(f) / SR) * envelope(n, 0.005, 0.12)
    m = int(0.08 * SR)
    tp = np.arange(m) / SR
    pop = np.sin(2 * np.pi * np.cumsum(500 * np.exp(-tp / 0.02) + 200) / SR) * envelope(m, 0.001, 0.02)
    out = w.copy()
    at = int(0.2 * SR)
    out[at:at + m] += pop[: n - at]
    return normalized(out, 0.7)


def grab():
    """つかんだ。短い音を2つ。"""
    n = int(0.25 * SR)
    out = np.zeros(n)
    for i, f0 in enumerate([1100, 1400]):
        m = int(0.06 * SR)
        t = np.arange(m) / SR
        blip = np.sin(2 * np.pi * f0 * t) * envelope(m, 0.002, 0.018)
        at = int(i * 0.09 * SR)
        out[at:at + m] += blip
    return normalized(out, 0.6)


def serve():
    """ぽん、と舟皿に乗って、きらきら。"""
    out = np.zeros(int(1.2 * SR))
    m = int(0.15 * SR)
    t = np.arange(m) / SR
    pop = np.sin(2 * np.pi * np.cumsum(150 + 250 * np.exp(-t / 0.03)) / SR) * envelope(m, 0.001, 0.05)
    out[:m] += pop
    for i, nt in enumerate([86, 90, 93, 98]):
        g = glock(nt, 0.15)
        at = int((0.1 + i * 0.08) * SR)
        g = g[: len(out) - at]
        out[at:at + len(g)] += g * 0.4
    return normalized(out, 0.9)


def slip():
    """ぽとっ。落ちる低い音と、少しのじゅっ。"""
    n = int(0.5 * SR)
    t = np.arange(n) / SR
    f = 120 + 280 * np.exp(-t / 0.04)
    thud = np.sin(2 * np.pi * np.cumsum(f) / SR) * envelope(n, 0.001, 0.07)
    s = sizzle(0.3, 0.4) * envelope(int(0.3 * SR), 0.01, 0.1)
    at = int(0.08 * SR)
    thud[at:at + len(s)] += s[: n - at]
    return normalized(thud, 0.7)


def fanfare():
    """ぴーひゃら、ドンドン。最後は和音をのばす。"""
    seq = [("A5", .5), ("B5", .5), ("D6", .5), ("B5", .5), ("A5", .5), ("D6", 2.0)]
    total = sum(b for _, b in seq) * BEAT + 2.2
    out = np.zeros(int(total * SR) + SR)
    t = 0.0
    for name, beats in seq:
        sig = fue(N[name], beats * BEAT * 0.95)
        i = int(t * SR)
        out[i:i + len(sig)] += sig * 0.35
        t += beats * BEAT
    for b in range(4):
        d = taiko()
        i = int(b * BEAT * SR)
        out[i:i + len(d)] += d * 0.6
    i = int((t - 2.0 * BEAT) * SR)
    for nt in (62, 66, 69, 74):
        p = pluck(nt, 2.0)
        out[i:i + len(p)] += p * 0.12
    c = chappa()
    out[i:i + len(c)] += c * 0.3
    d = taiko()
    out[i:i + len(d)] += d * 0.7
    out = out[: int(total * SR)]
    return normalized(out, 0.85)


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
    write("pour", pour())
    write("sizzle", poke())
    write("ready", ready())
    write("flip", flip())
    write("grab", grab())
    write("serve", serve())
    write("slip", slip())
    write("fanfare", fanfare())


if __name__ == "__main__":
    main()
