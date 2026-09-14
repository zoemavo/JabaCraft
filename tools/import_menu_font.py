"""Convert Faithful's ASCII bitmap to a scalable pixel-outline font.
Usage: python tools/import_menu_font.py /path/to/Faithful-32x-Java
Requires Pillow and fonttools. Source artwork remains under the Faithful License.
"""
import sys
from pathlib import Path
from PIL import Image
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

source = Path(sys.argv[1]) / "assets/minecraft/textures/font/ascii.png"
image = Image.open(source).convert("RGBA")
cell = image.width // 16
scale = 1024 // cell
names = [".notdef"] + [f"uni{code:04X}" for code in range(32, 127)]
glyphs, metrics = {}, {}
for code, name in [(0, ".notdef")] + [(code, f"uni{code:04X}") for code in range(32, 127)]:
    pen = TTGlyphPen(None)
    right = 0
    if code:
        for y in range(cell):
            for x in range(cell):
                if image.getpixel(((code % 16) * cell + x, (code // 16) * cell + y))[3] > 128:
                    right = max(right, x + 1)
                    left, bottom = x * scale, (cell - 3 - y) * scale
                    pen.moveTo((left, bottom))
                    pen.lineTo((left, bottom + scale))
                    pen.lineTo((left + scale, bottom + scale))
                    pen.lineTo((left + scale, bottom))
                    pen.closePath()
    glyphs[name] = pen.glyph()
    metrics[name] = ((right + 2) * scale if right else 512, 0)
font = FontBuilder(1024, isTTF=True)
font.setupGlyphOrder(names)
font.setupCharacterMap({code: f"uni{code:04X}" for code in range(32, 127)})
font.setupGlyf(glyphs)
font.setupHorizontalMetrics(metrics)
font.setupHorizontalHeader(ascent=1024, descent=-256)
font.setupNameTable({"familyName": "Faithful Menu", "styleName": "Regular",
    "uniqueFontIdentifier": "JabaCraft Faithful Menu", "fullName": "Faithful Menu",
    "psName": "FaithfulMenu", "copyright": "Faithful Resource Pack — Faithful License"})
font.setupOS2(sTypoAscender=1024, sTypoDescender=-256, usWinAscent=1024, usWinDescent=256)
font.setupPost()
font.setupMaxp()
font.save("assets/ui/menu/faithful.ttf")
