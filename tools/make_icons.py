"""Genera todos los iconos de SyncroUmbral a partir del logo maestro (docs/brand/logo-source.jpg).

Uso:  python tools/make_icons.py
Salidas: icono redondeado para el PC (tauri icon), recursos de Android (icono adaptativo, monocromo, icono de notificación)
y PNG para las interfaces. El logo fuente se guarda en docs/brand para poder regenerar todo.
"""
import os
import sys
from collections import deque

from PIL import Image, ImageFilter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "docs", "brand", "logo-source.jpg")
RES = os.path.join(ROOT, "android", "app", "src", "main", "res")
UI = os.path.join(ROOT, "desktop", "ui")
BUILD = os.path.join(ROOT, "docs", "brand", "generated")
os.makedirs(BUILD, exist_ok=True)

img = Image.open(SRC).convert("RGB")
W, H = img.size
px = img.load()


def near_white(p):
    return p[0] > 225 and p[1] > 225 and p[2] > 225


# 1) Esquinas: relleno desde los bordes sobre los píxeles casi blancos (lo de fuera del cuadrado redondeado).
outside = Image.new("L", (W, H), 0)
op = outside.load()
dq = deque()
for x in range(W):
    for y in (0, H - 1):
        dq.append((x, y))
for y in range(H):
    for x in (0, W - 1):
        dq.append((x, y))
while dq:
    x, y = dq.popleft()
    if x < 0 or y < 0 or x >= W or y >= H or op[x, y] or not near_white(px[x, y]):
        continue
    op[x, y] = 255
    dq.extend(((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)))

# Borde limpio: se ensancha un poco lo exterior (quita el halo claro del JPG) y se suaviza.
outside = outside.filter(ImageFilter.MaxFilter(5)).filter(ImageFilter.GaussianBlur(1.4))
alpha = outside.point(lambda v: 255 - v)
rounded = img.copy()
rounded.putalpha(alpha)
rounded.resize((1024, 1024), Image.LANCZOS).save(os.path.join(BUILD, "logo-rounded-1024.png"))

# 2) Color de fondo del logo (muestras del interior, lejos del dibujo).
samples = [px[int(W * fx), int(H * fy)] for fx, fy in ((0.18, 0.18), (0.82, 0.18), (0.18, 0.82), (0.82, 0.82), (0.5, 0.1), (0.5, 0.92))]
bg = tuple(sum(s[i] for s in samples) // len(samples) for i in range(3))
print("fondo", "#%02x%02x%02x" % bg)

# 3) Solo el dibujo (sin fondo): la transparencia sale de cuánto se aleja cada píxel del color de fondo.
glyph = Image.new("RGBA", (W, H), (0, 0, 0, 0))
gp = glyph.load()
ap = alpha.load()
for y in range(H):
    for x in range(W):
        if ap[x, y] < 250:
            continue
        r, g, b = px[x, y]
        d = max(abs(r - bg[0]), abs(g - bg[1]), abs(b - bg[2]))
        a = max(0, min(255, (d - 14) * 255 // 40))
        if a:
            gp[x, y] = (r, g, b, a)
# Recuadro del dibujo: solo píxeles claramente opacos y lejos del borde (el JPG deja ruido en las esquinas).
core = glyph.getchannel("A").point(lambda v: 255 if v > 120 else 0)
margin = int(W * 0.08)
core.paste(0, (0, 0, W, margin))
core.paste(0, (0, H - margin, W, H))
core.paste(0, (0, 0, margin, H))
core.paste(0, (W - margin, 0, W, H))
bbox = core.getbbox()
print("caja del dibujo", bbox)
# Se limpia lo que quede fuera del recuadro (ruido)
clean = Image.new("RGBA", glyph.size, (0, 0, 0, 0))
clean.paste(glyph.crop(bbox), bbox[:2])
glyph = clean
glyph = glyph.crop(bbox)
gw, gh = glyph.size


def on_canvas(size, width_fraction, im=glyph):
    """Centra el dibujo en un lienzo transparente ocupando `width_fraction` del ancho."""
    s = size * width_fraction / max(im.size)
    g = im.resize((max(1, round(im.width * s)), max(1, round(im.height * s))), Image.LANCZOS)
    c = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    c.paste(g, ((size - g.width) // 2, (size - g.height) // 2), g)
    return c


# 4) Android: icono adaptativo (108 dp; zona segura central ~66 %), monocromo para iconos con tema y icono de notificación.
os.makedirs(os.path.join(RES, "drawable-nodpi"), exist_ok=True)
fg = on_canvas(432, 0.60)
fg.save(os.path.join(RES, "drawable-nodpi", "ic_launcher_foreground.png"))

mono = Image.new("RGBA", fg.size, (0, 0, 0, 0))
mono.putalpha(fg.getchannel("A"))
mono_white = Image.new("RGBA", fg.size, (255, 255, 255, 255))
mono_white.putalpha(fg.getchannel("A"))
mono_white.save(os.path.join(RES, "drawable-nodpi", "ic_launcher_monochrome.png"))

# Icono de la barra de estado: solo la parte cian (el símbolo de sincronización), en silueta blanca y a 24 dp.
cyan = Image.new("RGBA", glyph.size, (0, 0, 0, 0))
cp, sp = cyan.load(), glyph.load()
for y in range(gh):
    for x in range(gw):
        r, g, b, a = sp[x, y]
        if a > 0 and g > 170 and b > 190:  # cian vivo; el gris azulado del monitor y el móvil se descarta
            cp[x, y] = (255, 255, 255, a)
cb = cyan.getbbox() or (0, 0, gw, gh)
stat = on_canvas(96, 0.92, cyan.crop(cb))
for d, size in (("mdpi", 24), ("hdpi", 36), ("xhdpi", 48), ("xxhdpi", 72), ("xxxhdpi", 96)):
    os.makedirs(os.path.join(RES, "drawable-" + d), exist_ok=True)
    stat.resize((size, size), Image.LANCZOS).save(os.path.join(RES, "drawable-" + d, "ic_stat_syncro.png"))

# Logo para la cabecera de la app del móvil.
on_canvas(256, 1.0).save(os.path.join(RES, "drawable-nodpi", "ic_logo.png"))

# Color de fondo del icono adaptativo.
os.makedirs(os.path.join(RES, "values"), exist_ok=True)
with open(os.path.join(RES, "values", "ic_launcher_colors.xml"), "w", encoding="utf-8") as f:
    f.write('<?xml version="1.0" encoding="utf-8"?>\n<resources>\n    <color name="ic_launcher_background">#%02X%02X%02X</color>\n</resources>\n' % bg)
os.makedirs(os.path.join(RES, "mipmap-anydpi-v26"), exist_ok=True)
adaptive = (
    '<?xml version="1.0" encoding="utf-8"?>\n'
    '<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">\n'
    '    <background android:drawable="@color/ic_launcher_background" />\n'
    '    <foreground android:drawable="@drawable/ic_launcher_foreground" />\n'
    '    <monochrome android:drawable="@drawable/ic_launcher_monochrome" />\n'
    '</adaptive-icon>\n'
)
for name in ("ic_launcher.xml", "ic_launcher_round.xml"):
    with open(os.path.join(RES, "mipmap-anydpi-v26", name), "w", encoding="utf-8") as f:
        f.write(adaptive)

# 5) Interfaz del PC y documentación: el logo completo (con su cuadrado redondeado).
final = Image.open(os.path.join(BUILD, "logo-rounded-1024.png"))
final.resize((128, 128), Image.LANCZOS).save(os.path.join(UI, "logo.png"))
final.resize((512, 512), Image.LANCZOS).save(os.path.join(ROOT, "docs", "brand", "logo-512.png"))
print("listo")
