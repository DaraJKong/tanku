"""Makes Tanku Lettering, the font of the interface, from the strokes that the floor is lettered with.

    python3 lettering_font.py [--specimen OUT.png]

It reads `tanku_client/assets/fonts/tanku_lettering/strokes.txt` and writes
`TankuLettering-Regular.ttf` next to it. Each stroke becomes its outline as the pen of the
drawing leaves it, a tenth of the height of a capital wide with round ends. A capital is one
em high, so that the size of a text is the height of its capitals, as h is in ISO 3098.

The name of the font carries a hash of the strokes that it was made from, and a test of the
game checks it: a font that was not made again after a change of the strokes fails it.

With `--specimen`, it also draws every character of the font, and a few lines at the sizes of
the interface, into a picture, as a reader sees them.

It needs fontTools, and Pillow for the specimen.
"""

import math
import os
import sys

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

HERE = os.path.dirname(os.path.abspath(__file__))
FOLDER = os.path.join(HERE, "..", "assets", "fonts", "tanku_lettering")
STROKES = os.path.join(FOLDER, "strokes.txt")
FONT = os.path.join(FOLDER, "TankuLettering-Regular.ttf")

# Font units in a tenth of the height of a capital: capitals are 1000 high, one em.
UNIT = 100
# The pen and the gap between two characters, in tenths.
PEN = 1.0
GAP = 2.0
# The steps in which an arc is drawn, in degrees, as the game draws them.
ARC_STEP = 7.5
# How far a mark stands clear of the top of what it marks: half a pen and a pen, in tenths.
CLEAR = 1.5
# Sides of the round end of a stroke.
ROUND = 16
# The ascent is the descent and one em, so that capitals stand in the middle of any line.
ASCENT, DESCENT = 1300, 300


def hashed(text):
    """FNV-1a of the strokes, with the ends of lines as Unix writes them: the test of the game
    hashes them the same way."""
    value = 0xCBF29CE484222325
    for byte in text.replace("\r\n", "\n").encode("utf-8"):
        value ^= byte
        value = (value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return "%016x" % value


def character(spec):
    if spec.startswith("U+") and len(spec) > 2:
        return chr(int(spec[2:], 16))
    if len(spec) != 1:
        raise ValueError("not a character: %r" % spec)
    return spec


def strokes(tokens):
    """The lines of a path, as `lettering.rs` reads them."""
    lines = []
    for token in tokens:
        kind, numbers = token[0], [float(n) for n in token[1:].split(",")]
        if kind == "M":
            lines.append([(numbers[0], numbers[1])])
        elif kind == "L":
            lines[-1].append((numbers[0], numbers[1]))
        elif kind == "A":
            x, y, across, up, start, end = numbers
            steps = max(4, math.ceil(abs(end - start) / ARC_STEP))
            points = []
            for step in range(steps + 1):
                angle = math.radians(start + (end - start) * step / steps)
                points.append((x + across * math.cos(angle), y + up * math.sin(angle)))
            if lines and math.dist(lines[-1][-1], points[0]) < 0.6:
                lines[-1].extend(points)
            else:
                lines.append(points)
        else:
            raise ValueError("not a stroke: %r" % token)
    return lines


def read(path):
    """The characters, the marks and the characters made of others."""
    glyphs, marks, made = {}, {}, {}
    with open(path, encoding="utf-8") as file:
        for number, line in enumerate(file, 1):
            words = line.split()
            # The number sign is written U+0023, so that a line that starts with one is a note.
            if not words or line.startswith("#"):
                continue
            try:
                if words[0] == "mark":
                    marks[words[1]] = (words[2], strokes(words[3:]))
                elif len(words) > 1 and words[1] == "=":
                    width, parts = None, []
                    for word in words[2:]:
                        if word.startswith("w") and len(word) > 1 and word[1].isdigit():
                            width = float(word[1:])
                        else:
                            parts.append(word)
                    made[character(words[0])] = (parts, width)
                else:
                    glyphs[character(words[0])] = (float(words[1]), strokes(words[2:]))
            except (ValueError, IndexError) as error:
                raise SystemExit("%s:%d: %s" % (path, number, error))
    return glyphs, marks, made


def placed(lines, sx, sy, dx, dy):
    return [[(sx * x + dx, sy * y + dy) for x, y in line] for line in lines]


def resolve(glyphs, marks, made):
    """Every character with its width and its lines, those made of others drawn whole."""
    done = dict(glyphs)

    def get(char):
        if char in done:
            return done[char]
        if char not in made:
            raise SystemExit("no character %r" % char)
        parts, width = made[char]
        # The first part is what the others are placed on, unless it is placed itself.
        first, transform = part_name(parts[0])
        if first in marks or transform is not None:
            base_width, base = 0.0, []
        else:
            (base_width, base), parts = get(first), parts[1:]
        lines = [list(line) for line in base]
        top = max((y for line in base for _, y in line), default=0)
        for part in parts:
            name, transform = part_name(part)
            if name in marks:
                kind, mark = marks[name]
                if transform is None:
                    transform = (1, 1, base_width / 2, top + CLEAR if kind == "above" else 0)
                lines += placed(mark, *transform)
            else:
                if transform is None:
                    raise SystemExit("%s: %s needs a place" % (char, part))
                lines += placed(get(name)[1], *transform)
        done[char] = (base_width if width is None else width, lines)
        return done[char]

    for char in made:
        get(char)
    return done


def part_name(part):
    """The name of a part, and where its numbers put it, if they do."""
    at = part.find("@", 1)
    if at < 0:
        name, transform = part, None
    else:
        name, transform = part[:at], tuple(float(n) for n in part[at + 1:].split(","))
    if len(name) == 1 or name.startswith("U+"):
        name = character(name)
    return name, transform


def contour(points):
    """A closed outline, clockwise as TrueType wants its outer ones."""
    area = sum(a[0] * b[1] - b[0] * a[1] for a, b in zip(points, points[1:] + points[:1]))
    return points[::-1] if area > 0 else points


def outline(lines, shift):
    """The outlines that the pen leaves along these lines: a band along each piece, the round
    of the pen on the outside of each joint, and a round end at each end."""
    radius = PEN / 2 * UNIT
    shapes = []

    def arc(center, start, end):
        """Points on the round of the pen, from one angle to the other the short way."""
        turn = (end - start + math.pi) % (2 * math.pi) - math.pi
        steps = max(1, math.ceil(abs(turn) / (2 * math.pi / ROUND)))
        return [(center[0] + radius * math.cos(start + turn * step / steps),
                 center[1] + radius * math.sin(start + turn * step / steps)) for step in range(steps + 1)]

    def half_round(center, start):
        """Half the round of the pen, counterclockwise from an angle."""
        steps = ROUND // 2
        return [(center[0] + radius * math.cos(start + math.pi * step / steps),
                 center[1] + radius * math.sin(start + math.pi * step / steps)) for step in range(steps + 1)]

    for line in lines:
        points = [((x + shift) * UNIT, y * UNIT) for x, y in line]
        kept = [points[0]]
        for point in points[1:]:
            if math.dist(point, kept[-1]) > 1.0:
                kept.append(point)
        if len(kept) == 1:
            x, y = kept[0]
            shapes.append([(x + radius * math.cos(2 * math.pi * side / ROUND), y + radius * math.sin(2 * math.pi * side / ROUND)) for side in range(ROUND)])
            continue
        closed = math.dist(kept[0], kept[-1]) < 2.0
        if closed:
            kept[-1] = kept[0]
        directions = [math.atan2(b[1] - a[1], b[0] - a[0]) for a, b in zip(kept, kept[1:])]
        for (a, b), direction in zip(zip(kept, kept[1:]), directions):
            nx, ny = -math.sin(direction) * radius, math.cos(direction) * radius
            shapes.append([(a[0] + nx, a[1] + ny), (b[0] + nx, b[1] + ny), (b[0] - nx, b[1] - ny), (a[0] - nx, a[1] - ny)])
        joints = list(zip(kept[1:-1], directions, directions[1:]))
        if closed:
            joints.append((kept[0], directions[-1], directions[0]))
        for point, before, after in joints:
            turn = (after - before + math.pi) % (2 * math.pi) - math.pi
            if abs(turn) < 1e-3:
                continue
            # The outside of the joint is to the right of a turn to the left, and so on.
            side = -math.pi / 2 if turn > 0 else math.pi / 2
            shapes.append([point] + arc(point, before + side, after + side))
        if not closed:
            # The round of the pen behind the first point and ahead of the last one.
            shapes.append(half_round(kept[0], directions[0] + math.pi / 2))
            shapes.append(half_round(kept[-1], directions[-1] - math.pi / 2))
    return [contour(shape) for shape in shapes if len(shape) >= 3]


def name_of(char):
    return "uni%04X" % ord(char) if ord(char) <= 0xFFFF else "u%05X" % ord(char)


def build(path, out):
    text = open(path, encoding="utf-8").read()
    glyphs, marks, made = read(path)
    every = resolve(glyphs, marks, made)
    order, cmap, outlines, metrics = [".notdef"], {}, {}, {}

    pen = TTGlyphPen(None)
    for box in ([(50, 0), (50, 1000), (650, 1000), (650, 0)], [(130, 80), (570, 80), (570, 920), (130, 920)]):
        pen.moveTo(box[0])
        for point in box[1:]:
            pen.lineTo(point)
        pen.closePath()
    outlines[".notdef"], metrics[".notdef"] = pen.glyph(), (700, 50)

    for char in sorted(every):
        width, lines = every[char]
        name = name_of(char)
        pen = TTGlyphPen(None)
        left = None
        # Half the gap, and half a pen, on the left of the first line.
        for shape in outline(lines, GAP / 2 + PEN / 2):
            rounded = [(round(x), round(y)) for x, y in shape]
            pen.moveTo(rounded[0])
            for point in rounded[1:]:
                pen.lineTo(point)
            pen.closePath()
            least = min(x for x, _ in rounded)
            left = least if left is None else min(left, least)
        outlines[name] = pen.glyph()
        metrics[name] = (round((width + PEN + GAP) * UNIT), left or 0)
        order.append(name)
        cmap[ord(char)] = name
    # Characters that are typed for others that the font has.
    for typed, drawn in [(" ", " "), ("‘", "'"), ("’", "'"), ("“", '"'), ("”", '"'), ("‐", "-"), ("‑", "-")]:
        cmap.setdefault(ord(typed), name_of(drawn))

    builder = FontBuilder(1000, isTTF=True)
    builder.setupGlyphOrder(order)
    builder.setupCharacterMap(cmap)
    builder.setupGlyf(outlines)
    builder.setupHorizontalMetrics(metrics)
    builder.setupHorizontalHeader(ascent=ASCENT, descent=-DESCENT, lineGap=0)
    version = "Version 1.0; strokes %s" % hashed(text)
    builder.setupNameTable({
        "familyName": "Tanku Lettering",
        "styleName": "Regular",
        "uniqueFontIdentifier": "TankuLettering-Regular; %s" % hashed(text),
        "fullName": "Tanku Lettering Regular",
        "psName": "TankuLettering-Regular",
        "version": version,
    })
    builder.setupOS2(
        sTypoAscender=ASCENT,
        sTypoDescender=-DESCENT,
        sTypoLineGap=0,
        usWinAscent=1450,
        usWinDescent=450,
        sxHeight=700,
        sCapHeight=1000,
        usWeightClass=400,
        achVendID="TNKU",
        fsType=0,
        version=4,
        # Regular, and the typographic ascent and descent are the ones to use.
        fsSelection=(1 << 6) | (1 << 7),
    )
    builder.setupPost()
    builder.save(out)
    return len(order)


def specimen(font, out):
    from PIL import Image, ImageDraw, ImageFont

    glyphs, marks, made = read(STROKES)
    chars = sorted(set(glyphs) | set(made))
    picture = Image.new("RGB", (1900, 1500), (10, 15, 22))
    draw = ImageDraw.Draw(picture)
    y = 20
    big = ImageFont.truetype(font, 42)
    row = ""
    rows = []
    for char in chars:
        if char == " ":
            continue
        row += char + " "
        if len(row) > 70:
            rows.append(row)
            row = ""
    rows.append(row)
    for row in rows:
        draw.text((20, y + 20), row, font=big, fill=(230, 235, 242))
        y += 72
    for size, line in [
        (84, "Tanku"),
        (59, "Lobby  Game over"),
        (42, "Sector 188  16"),
        (29.7, "TANK DESTROYED  Last life"),
        (21, "Solo · normal  K23NFVN7  0.8.0  Retry this seed  Illinois 1lI0O"),
        (14.8, "Seed  Lives  Barrier 4 · open  Type a seed of your own, in letters and digits."),
        (14.8, "Soirée chez Dara · Łódź · Ærøskøbing · Smørrebrød · Ångström · ß · Œuvre · Żółć"),
    ]:
        face = ImageFont.truetype(font, size)
        y += round(size * 0.8)
        draw.text((20, y), line, font=face, fill=(230, 235, 242))
        draw.line([(10, y + round(size * 1.3)), (14, y + round(size * 1.3))], fill=(242, 77, 64))
        y += round(size * 1.2)
    picture.save(out)


if __name__ == "__main__":
    count = build(STROKES, FONT)
    print("%d glyphs in %s" % (count, os.path.relpath(FONT)))
    if len(sys.argv) == 3 and sys.argv[1] == "--specimen":
        specimen(FONT, sys.argv[2])
        print("specimen in %s" % sys.argv[2])
