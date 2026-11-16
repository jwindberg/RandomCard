"""Make the Tour's movie, src/model/tour_movie/Card Trick.mp4: a playing card
turning over on a table, with a little tune, and the time in the corner so
seeking and stepping can be seen. H.264 and AAC in MP4, which every system's
movie player reads.

usage: make_tour_movie.py

Needs Python with Pillow, and GStreamer's command line (gst-launch-1.0) with
the openh264 and libav plugins.
"""
import math
import os
import struct
import subprocess
import sys
import tempfile
import wave

from PIL import Image, ImageDraw, ImageFont

W, H, FPS, SECONDS = 320, 240, 24, 4
RATE = 44100
OUT = os.path.join(os.path.dirname(__file__), '..', 'src', 'model', 'tour_movie', 'Card Trick.mp4')


def font(size):
    try:
        return ImageFont.load_default(size=size)
    except TypeError:
        return ImageFont.load_default()


def heart(d, cx, cy, s, fill):
    """A heart s across, centered at cx, cy."""
    r = s / 4
    d.ellipse([cx - 2 * r, cy - r * 1.6, cx, cy + r * 0.4], fill=fill)
    d.ellipse([cx, cy - r * 1.6, cx + 2 * r, cy + r * 0.4], fill=fill)
    d.polygon([(cx - 2 * r + 1, cy - r * 0.4), (cx + 2 * r - 1, cy - r * 0.4), (cx, cy + 2 * r)], fill=fill)


def card_face(front):
    """The card, 120 by 168, face up (the ace of hearts) or down."""
    c = Image.new('RGBA', (120, 168), (0, 0, 0, 0))
    d = ImageDraw.Draw(c)
    d.rounded_rectangle([0, 0, 119, 167], 10, fill=(255, 255, 255), outline=(30, 30, 30), width=2)
    if front:
        red = (200, 30, 40)
        heart(d, 60, 80, 56, red)
        f = font(20)
        d.text((10, 6), 'A', font=f, fill=red)
        heart(d, 17, 40, 14, red)
        d.text((100, 140), 'A', font=f, fill=red)
    else:
        d.rounded_rectangle([8, 8, 111, 159], 6, fill=(30, 80, 170))
        for y in range(14, 156, 10):
            for x in range(14, 108, 10):
                d.ellipse([x, y, x + 4, y + 4], fill=(240, 200, 60))
    return c


def frame(n, front, back):
    t = n / FPS
    img = Image.new('RGB', (W, H))
    d = ImageDraw.Draw(img)
    for y in range(H):
        g = int(90 + 50 * y / H)
        d.line([(0, y), (W, y)], fill=(20, g, 80))
    # The card rests, then turns over in the last part of each second:
    # face down, face up, face down, face up.
    k, phase = int(t), t % 1
    p = max(0.0, (phase - 0.6) / 0.4)
    faces = [back, front]
    shown = faces[(k + (p >= 0.5)) % 2]
    squeeze = abs(math.cos(math.pi * p))
    lift = int(18 * math.sin(math.pi * p))
    wide = max(2, int(120 * squeeze))
    card = shown.resize((wide, 168))
    d.ellipse([W / 2 - 64, 200, W / 2 + 64, 216], fill=(10, 60, 40))
    img.paste(card, (W // 2 - wide // 2, 28 - lift), card)
    f = font(14)
    stamp = f'{t:.2f}'
    d.rounded_rectangle([W - 52, H - 24, W - 6, H - 6], 4, fill=(0, 0, 0))
    d.text((W - 46, H - 22), stamp, font=f, fill=(255, 255, 255))
    d.text((8, H - 22), 'RandomCard', font=f, fill=(255, 255, 255))
    return img


def tune(path):
    """C, E, G and a high C, a note each second, gently plucked."""
    notes = [261.63, 329.63, 392.0, 523.25]
    frames = []
    for i in range(RATE * SECONDS):
        t = i / RATE
        n = int(t)
        dt = t - n
        v = 0.3 * math.exp(-3 * dt) * (math.sin(2 * math.pi * notes[n] * t) + 0.3 * math.sin(4 * math.pi * notes[n] * t))
        frames.append(struct.pack('<h', int(max(-1, min(1, v)) * 32767)))
    with wave.open(path, 'wb') as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b''.join(frames))


def main():
    front, back = card_face(True), card_face(False)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        for n in range(FPS * SECONDS):
            frame(n, front, back).save(os.path.join(tmp, f'{n:04d}.png'))
        tune(os.path.join(tmp, 'tune.wav'))
        subprocess.run([
            'gst-launch-1.0', '-q',
            'multifilesrc', f'location={tmp}/%04d.png', 'index=0', f'caps=image/png,framerate={FPS}/1',
            '!', 'pngdec', '!', 'videoconvert', '!', 'video/x-raw,format=I420',
            '!', 'openh264enc', 'bitrate=400000', 'complexity=high', '!', 'h264parse',
            '!', 'mp4mux', 'faststart=true', 'name=m', '!', 'filesink', f'location={OUT}',
            'filesrc', f'location={tmp}/tune.wav', '!', 'wavparse', '!', 'audioconvert', '!', 'audioresample',
            '!', 'avenc_aac', 'bitrate=64000', '!', 'aacparse', '!', 'm.',
        ], check=True)
    print(OUT, os.path.getsize(OUT), 'bytes')


if __name__ == '__main__':
    sys.exit(main())
