import json
import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

FONT = "/System/Library/Fonts/Helvetica.ttc"
HEIGHT = 1400
HEADER = 230
FPS = 10
OUT_FPS = 60
GAP = 40
TAIL = 1.5
BG = (17, 17, 17)


def duration(path):
    out = subprocess.check_output(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration:stream=width,height", "-of", "json", path]
    )
    info = json.loads(out)
    stream = info["streams"][0]
    return float(info["format"]["duration"]), stream["width"], stream["height"]


def first_change(path):
    width, height = 32, 64
    raw = subprocess.check_output(
        ["ffmpeg", "-v", "error", "-i", path, "-vf", f"fps=20,scale={width}:{height},format=gray", "-f", "rawvideo", "-"]
    )
    size = width * height
    frames = [raw[i:i + size] for i in range(0, len(raw) - size + 1, size)]
    first = frames[0]
    for index, frame in enumerate(frames[1:], start=1):
        changed = sum(1 for a, b in zip(first, frame) if abs(a - b) > 24)
        if changed > size * 0.02:
            return index / 20
    return 0.0


def timer(t):
    tenths = int(t * 10)
    return f"{tenths // 600:02d}:{(tenths // 10) % 60:02d}.{tenths % 10}"


def render_header(folder, width, label, own, total):
    folder.mkdir(parents=True, exist_ok=True)
    title = ImageFont.truetype(FONT, 34)
    clock = ImageFont.truetype(FONT, 88)
    badge = ImageFont.truetype(FONT, 40)
    frames = int(total * FPS) + 1
    for index in range(frames):
        t = index / FPS
        image = Image.new("RGB", (width, HEADER), BG)
        draw = ImageDraw.Draw(image)
        draw.text((width / 2, 42), label, font=title, fill="white", anchor="mm")
        colour = (6, 214, 160) if t >= own else (255, 209, 102)
        draw.text((width / 2, 130), timer(min(t, own)), font=clock, fill=colour, anchor="mm")
        if t >= own:
            draw.text((width / 2, 200), "DONE", font=badge, fill=(6, 214, 160), anchor="mm")
        image.save(folder / f"{index:05d}.png")


def main(left, right, output):
    work = Path(output).with_suffix("") .parent / ".compose"
    shutil.rmtree(work, ignore_errors=True)
    sides = []
    total = 0.0
    for path, label in (left, right):
        own, w, h = duration(path)
        lead = first_change(path)
        own -= lead
        total = max(total, own)
        sides.append((path, label, own, w * HEIGHT // h // 2 * 2, lead))
    total += TAIL
    inputs = []
    chains = []
    for index, (path, label, own, width, lead) in enumerate(sides):
        folder = work / f"header{index}"
        render_header(folder, width, label, own, total)
        inputs += ["-i", path, "-framerate", str(FPS), "-i", str(folder / "%05d.png")]
        video = 2 * index
        chains.append(
            f"[{video}:v]fps={OUT_FPS}:start_time=0,trim=start={lead:.3f},setpts=PTS-STARTPTS,scale={width}:{HEIGHT},"
            f"tpad=stop_mode=clone:stop_duration={total - own:.3f}[v{index}];"
            f"[{video + 1}:v]fps={OUT_FPS}[h{index}];[h{index}][v{index}]vstack[s{index}]"
        )
    graph = ";".join(chains) + (
        f";[s0]pad=iw+{GAP}:ih:0:0:color=0x111111[g0];[g0][s1]hstack,"
        f"pad=iw+{2 * GAP}:ih+{GAP}:{GAP}:0:color=0x111111[out]"
    )
    subprocess.check_call(
        ["ffmpeg", "-y", "-v", "error", *inputs, "-filter_complex", graph, "-map", "[out]", "-t", f"{total:.3f}",
         "-r", str(OUT_FPS), "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "20", "-movflags", "+faststart", output]
    )
    shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main(
        (sys.argv[1], sys.argv[2]),
        (sys.argv[3], sys.argv[4]),
        sys.argv[5],
    )
