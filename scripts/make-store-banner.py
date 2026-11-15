"""Builds the Snap Store banner (1920x640, 3:1) from the app icon and the README screenshots.

Usage: python3 scripts/make-store-banner.py [output.png]   (default: docs/store/banner.png)
Needs Pillow, pycairo, librsvg (gir1.2-rsvg-2.0) and the Inter font.
"""
import os
import sys

import gi
gi.require_version("Rsvg", "2.0")
import cairo
from gi.repository import Rsvg
from PIL import Image, ImageDraw, ImageFilter, ImageFont

# The project root (this script lives in scripts/).
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "docs", "store", "banner.png")
W, H = 1920, 640
FONTS = "/usr/share/fonts/truetype/inter-zorin-os/"


def lerp(a, b, t):
    return tuple(int(x + (y - x) * t) for x, y in zip(a, b))


# Background: deep teal-to-blue diagonal gradient, ThermalPrint's colours, with a
# soft green glow behind the screenshots.
bg = Image.new("RGB", (W, H))
px = bg.load()
c1, c2 = (8, 30, 36), (16, 62, 78)
for y in range(H):
    for x in range(W):
        px[x, y] = lerp(c1, c2, min(1.0, (x / W) * 0.8 + (y / H) * 0.2))
glow = Image.new("L", (W, H), 0)
ImageDraw.Draw(glow).ellipse((1050, -150, 1950, 800), fill=120)
glow = glow.filter(ImageFilter.GaussianBlur(160))
bg = Image.composite(Image.new("RGB", (W, H), (36, 146, 126)), bg, glow).convert("RGBA")


def icon(size):
    handle = Rsvg.Handle.new_from_file(f"{ROOT}/data/org.thermalprint.ThermalPrint.svg")
    surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, size, size)
    ctx = cairo.Context(surface)
    rect = Rsvg.Rectangle()
    rect.x, rect.y, rect.width, rect.height = 0, 0, size, size
    handle.render_document(ctx, rect)
    # Cairo is premultiplied BGRA; Pillow wants straight RGBA.
    return Image.frombuffer("RGBA", (size, size), bytes(surface.get_data()), "raw", "BGRa", 0, 1)


def shot(path, height):
    im = Image.open(f"{ROOT}/docs/screenshots/{path}").convert("RGBA")
    w = round(im.width * height / im.height)
    return im.resize((w, height), Image.LANCZOS)


def shadow(im, offset=(0, 24), blur=36, alpha=150):
    mask = im.split()[3].point(lambda a: alpha if a > 40 else 0)
    pad = blur * 3
    sh = Image.new("RGBA", (im.width + pad * 2, im.height + pad * 2), (0, 0, 0, 0))
    sh.paste((0, 0, 0, 255), (pad + offset[0], pad + offset[1]), mask)
    return sh.filter(ImageFilter.GaussianBlur(blur)), pad


def place(canvas, im, x, y):
    sh, pad = shadow(im)
    canvas.alpha_composite(sh, (x - pad, y - pad)) if x - pad >= 0 else canvas.paste(sh, (x - pad, y - pad), sh)
    canvas.paste(im, (x, y), im)


# Two screenshots: Import & Convert behind (dark), the Label Designer in front;
# both run off the bottom edge so the banner feels like a window into the app.
back = shot("import-dark.png", 470)
back.putalpha(back.split()[3].point(lambda a: int(a * 0.85)))
front = shot("designer-light.png", 520)
layer = Image.new("RGBA", (W, H + 400), (0, 0, 0, 0))
place(layer, back, 1250, 52)
place(layer, front, 1020, 150)
bg.alpha_composite(layer.crop((0, 0, W, H)))

# Left: icon, name, tagline, feature pills.
draw = ImageDraw.Draw(bg)
bg.alpha_composite(icon(176), (118, 112))
title = ImageFont.truetype(FONTS + "Inter-Bold.ttf", 84)
tag = ImageFont.truetype(FONTS + "Inter-Medium.ttf", 36)
small = ImageFont.truetype(FONTS + "Inter-Medium.ttf", 27)
draw.text((122, 318), "ThermalPrint", font=title, fill=(255, 255, 255))
draw.text((124, 424), "Shipping labels and your own designs,", font=tag, fill=(204, 240, 230))
draw.text((124, 470), "printed right on Linux.", font=tag, fill=(204, 240, 230))

PILLS = ["Shipping Labels", "Label Designer", "Barcodes & QR"]
# Feature pills: translucent, so drawn on their own layer and blended.
pills = Image.new("RGBA", (W, H), (0, 0, 0, 0))
pd = ImageDraw.Draw(pills)
x = 124
for label in PILLS:
    tw = pd.textlength(label, font=small)
    pd.rounded_rectangle((x, 548, x + tw + 36, 592), radius=22, fill=(255, 255, 255, 28), outline=(255, 255, 255, 80), width=2)
    x += tw + 36 + 14
bg.alpha_composite(pills)
x = 124
for label in PILLS:
    tw = draw.textlength(label, font=small)
    draw.text((x + 18, 570), label, font=small, fill=(232, 250, 244), anchor="lm")
    x += tw + 36 + 14

os.makedirs(os.path.dirname(OUT), exist_ok=True)
bg.convert("RGB").save(OUT, "PNG", optimize=True)
print(OUT, bg.size)
