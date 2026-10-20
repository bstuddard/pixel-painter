#!/usr/bin/env python3
"""Reproduce the review gallery using only the painter CLI and Python's stdlib.

The destination must not exist. Nothing is deleted or overwritten. Every raster
pixel is created by painter operations; Python supplies recipes and orchestration.
"""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def op(kind, **fields):
    return {"op": kind, **fields}


def box(x, y, width, height, color, **fields):
    return op("fill_rect", x=x, y=y, width=width, height=height, color=color, **fields)


def ellipse(x, y, width, height, color, **fields):
    return op("ellipse", x=x, y=y, width=width, height=height, color=color, fill=True, **fields)


def polygon(points, color):
    return op("polygon", points=[{"x": x, "y": y} for x, y in points], color=color, fill=True)


class Gallery:
    def __init__(self, painter, root):
        self.painter = str(painter.resolve())
        self.root = root
        root.mkdir(parents=True, exist_ok=False)

    def run(self, *args, file=None, report=None):
        command = [self.painter, "--json"]
        if file:
            command += ["--file", str(file)]
        result = subprocess.run(command + [str(a) for a in args], capture_output=True, text=True, check=True)
        if report:
            report.write_text(result.stdout, encoding="utf-8")
        return json.loads(result.stdout)

    def batch(self, folder, name, operations, file, **fields):
        source = folder / (name + ".ops.json")
        write(source, {"version": 1, "operations": operations, **fields})
        self.run("batch", "--input", source, file=file)

    def project(self, folder, width, height, layers):
        folder.mkdir()
        path = folder / "project.painter.json"
        self.run("project", "new", path, "--width", width, "--height", height)
        for layer in layers:
            self.run("project", "layer", path, "add", layer)
        return path

    def layer_batch(self, folder, project, layer, operations, label):
        self.run("project", "layer", project, "select", layer)
        self.batch(folder, label, operations, project)

    def finish(self, folder, project, columns=1, scale=6):
        self.run("project", "frame", project, "select", 0)
        self.run("export", "--output", folder / "native.png", file=project)
        self.run("inspect", file=project, report=folder / "inspection.json")
        self.run("project", "export", project, "--output", folder / "export", "--columns", columns, "--padding", 2, "--margin", 2, "--scale", scale)

    def keeper(self):
        folder = self.root / "01-crystal-keeper"
        project = self.project(folder, 48, 48, ["cloak", "face", "staff"])
        dark, deep, mid, light = "#18243C", "#303B70", "#5864A0", "#929ED0"
        for frame, (direction, step) in enumerate((d, s) for d in ("south", "east", "north", "west") for s in range(2)):
            if frame:
                self.run("project", "frame", project, "add", f"{direction}-{step}", "--duration-ms", 180)
            else:
                self.run("project", "frame", project, "set", 0, "--duration-ms", 180)
            self.run("project", "frame", project, "set", frame, "--pivot-x", 24, "--pivot-y", 43)
            self.layer_batch(folder, project, "base", [ellipse(10, 40, 29, 5, "#17213965")], f"{frame:02}-shadow")
            feet = [box(16, 36 + step, 6, 6 - step, dark), box(26, 37 - step, 6, 5 + step, dark), box(16, 40 + step, 7, 2, "#6E5262"), box(26, 40 - step, 7, 2, "#6E5262")]
            cloak = feet + [polygon([(13, 31), (14, 15), (18, 8), (24, 5), (31, 9), (34, 17), (33, 33), (29, 39), (16, 38)], dark), polygon([(15, 31), (16, 16), (20, 10), (24, 8), (29, 11), (32, 18), (31, 32), (28, 36), (17, 36)], mid), polygon([(15, 31), (18, 23), (20, 27), (20, 36), (17, 36)], deep), polygon([(26, 25), (30, 22), (31, 31), (28, 35), (26, 35)], light), polygon([(17, 14), (21, 9), (24, 8), (20, 14)], light), box(16, 32, 15, 2, deep), box(22, 32, 5, 3, "#D8AC61"), box(23, 33, 3, 1, "#FFE6A0"), box(12, 27, 5, 6, dark), box(13, 28, 3, 4, mid), box(31, 27, 5, 6, dark), box(32, 28, 4, 3, mid)]
            if direction == "north":
                face = [polygon([(17, 16), (23, 11), (29, 16), (29, 23), (25, 27), (18, 23)], deep), box(21, 14, 2, 10, mid), box(17, 24, 14, 2, light), box(22, 26, 3, 4, "#D8AC61")]
            else:
                shift = 2 if direction == "east" else -2 if direction == "west" else 0
                face = [ellipse(17 + shift, 14, 14, 13, dark), ellipse(19 + shift, 16, 10, 8, "#D0A186"), box(20 + shift, 17, 8, 3, "#EDC7A0"), box(20 + shift, 20, 2, 2, dark), box(26 + shift, 20, 2, 2, dark), box(23 + shift, 23, 3, 1, "#8D626C"), box(16, 24, 16, 3, light), box(18, 26, 12, 2, mid), box(19, 28, 3, 3, light)]
            staff = [ellipse(31, 5, 15, 16, "#4AEDD52C"), box(36, 17, 3, 24, dark), box(37, 17, 1, 23, "#BC8B62"), polygon([(37, 5), (43, 12), (38, 19), (32, 12)], dark), polygon([(37, 7), (41, 12), (38, 16), (34, 12)], "#43CDBB"), polygon([(37, 7), (37, 13), (34, 12)], "#B3FFE0"), polygon([(38, 13), (41, 12), (38, 16)], "#278A9B"), box(35, 18, 6, 2, "#D8AC61"), box(35, 29, 5, 3, "#EDC7A0"), box(37, 9 + step, 1, 2, "#FFFFFF")]
            self.layer_batch(folder, project, "cloak", cloak, f"{frame:02}-cloak")
            self.layer_batch(folder, project, "face", face, f"{frame:02}-face")
            self.layer_batch(folder, project, "staff", staff, f"{frame:02}-staff")
        for i, direction in enumerate(("south", "east", "north", "west")):
            self.run("project", "tag", project, "set", "walk-" + direction, "--frames", f"{i*2},{i*2+1}", "--direction", direction)
        self.finish(folder, project, columns=4, scale=4)
        self.run("snapshot", "--output", folder / "checkpoint.painter.json", file=project)

    def cottage(self):
        folder = self.root / "02-moss-cottage"
        project = self.project(folder, 64, 64, ["walls", "roof", "details"])
        self.layer_batch(folder, project, "base", [ellipse(5, 49, 55, 10, "#193D3B85"), ellipse(7, 48, 51, 8, "#35634B"), ellipse(9, 48, 47, 5, "#527F4F"), ellipse(24, 55, 20, 4, "#63776C")], "ground")
        self.layer_batch(folder, project, "walls", [box(12, 29, 42, 25, "#263B43"), box(14, 30, 38, 22, "#CDAE80"), box(14, 32, 38, 3, "#B9916E"), box(14, 50, 38, 3, "#8D7765"), box(15, 36, 2, 13, "#F0D4A1"), box(48, 35, 3, 15, "#A68263"), box(29, 36, 12, 18, "#263B43"), box(31, 38, 8, 15, "#6B5552"), box(32, 39, 2, 12, "#9D7560"), box(37, 45, 1, 2, "#F9DA87"), box(27, 53, 16, 2, "#A0A68A"), box(25, 55, 20, 2, "#7B8B7C")], "walls")
        roof = [box(42, 8, 8, 16, "#263B43"), box(44, 9, 4, 12, "#A8766A"), box(41, 7, 10, 3, "#D5B897"), box(44, 14, 3, 1, "#714F55"), polygon([(6, 31), (30, 8), (34, 8), (59, 31), (57, 35), (8, 35)], "#263B43"), polygon([(9, 30), (31, 10), (33, 10), (56, 31), (9, 32)], "#477457")]
        for y, left, right in [(16, 25, 39), (21, 20, 44), (26, 14, 51), (31, 10, 55)]:
            roof += [box(left, y, right-left, 2, "#2E584B"), box(left+1, y-1, max(1, right-left-3), 1, "#78A267")]
            for x in range(left+3, right, 7):
                roof.append(box(x, y-3, 1, 3, "#2E584B"))
        roof += [box(29, 10, 5, 2, "#9CBD7C"), box(11, 32, 43, 2, "#263B43"), box(15, 31, 9, 2, "#739858"), box(43, 30, 8, 3, "#89A564")]
        self.layer_batch(folder, project, "roof", roof, "roof")
        details = []
        for x in (18, 43):
            details += [box(x, 37, 7, 9, "#263B43"), box(x+1, 38, 5, 6, "#E9BB71"), box(x+1, 38, 2, 3, "#FFE5A1"), box(x+3, 38, 1, 6, "#6B5552"), box(x+1, 41, 5, 1, "#6B5552"), box(x-1, 45, 9, 2, "#71584E"), box(x-1, 45, 9, 1, "#B28766")]
        for x, y in [(9, 51), (50, 51), (15, 55), (54, 48)]:
            details += [box(x, y-2, 1, 3, "#81AA64"), box(x-1, y-3, 3, 2, "#E9A687"), box(x, y-3, 1, 1, "#FFE4B4")]
        details += [box(8, 51, 1, 3, "#FFE4B4"), ellipse(5, 48, 7, 4, "#BC6A67"), box(7, 48, 1, 1, "#FFE4B4"), box(51, 12, 2, 1, "#D2DCCD90"), box(52, 8, 3, 2, "#D2DCCD60"), box(50, 4, 4, 2, "#D2DCCD30")]
        self.layer_batch(folder, project, "details", details, "windows-and-garden")
        self.finish(folder, project, scale=6)

    def tiles(self):
        folder = self.root / "03-forest-tiles"
        folder.mkdir()
        colors = {"edge": "#35664D", "grass": "#4E8052", "grass_light": "#72A15D", "grass_dark": "#416D4D", "stone": "#87918A", "stone_light": "#ACB4A0", "water": "#3A8193", "water_light": "#68AFAD"}
        palette = {"version": 1, "colors": colors}
        write(folder / "palette.json", palette)
        operations = [op("clear", color="edge")]
        for tile in range(8):
            x, y = (tile % 4)*16, (tile // 4)*16
            operations += [box(x+1, y+1, 14, 14, "grass"), op("noise", x=x+2, y=y+2, width=12, height=12, colors=["grass_light", "grass_dark"], density=65)]
            if tile in (2, 3, 6):
                operations += [ellipse(x+3, y+4, 10, 8, "grass_dark"), ellipse(x+4, y+3, 9, 8, "stone"), box(x+5, y+4, 5, 1, "stone_light"), box(x+8, y+9, 4, 1, "grass_dark")]
            if tile in (4, 5, 7):
                operations += [ellipse(x+2, y+3, 12, 10, "grass_dark"), ellipse(x+3, y+4, 10, 8, "water"), box(x+5, y+5, 5, 1, "water_light"), box(x+7, y+9, 4, 1, "water_light")]
        template = {"version": 2, "width": 64, "height": 32, "seed": 724, "palette": palette, "policy": "strict", "operations": operations}
        write(folder / "terrain.template.json", template)
        canvas = folder / "terrain.json"
        self.run("generate", "--input", folder / "terrain.template.json", file=canvas)
        self.run("export", "--output", folder / "native.png", file=canvas)
        self.run("render", "--output", folder / "preview.png", "--scale", 8, file=canvas)
        self.run("inspect", "--palette", folder / "palette.json", file=canvas, report=folder / "inspection.json")
        rules = {"version": 1, "connections": [{"from": i, "to": (i+1) % 8, "edge": edge} for i in range(8) for edge in ("right", "down")]}
        write(folder / "connections.json", rules)
        prefix = ["tiles", "--tile-width", 16, "--tile-height", 16]
        self.run(*prefix, "seams", "--rules", folder / "connections.json", file=canvas, report=folder / "seams.json")
        self.run(*prefix, "export", "--output", folder / "export", "--columns", 4, "--padding", 2, "--margin", 2, "--scale", 8, "--rules", folder / "connections.json", file=canvas)

    def slimes(self):
        folder = self.root / "04-seeded-slimes"
        folder.mkdir()
        palette = {"version": 1, "colors": {"outline": "#193F4A", "body": "#47AD91", "shade": "#2C7C79", "light": "#84D9A0", "shine": "#D8F5BB", "eyes": "#193F4A"}}
        operations = [ellipse(3, 19, 19, 4, "#193F4A50"), ellipse(3, 5, 19, 17, "outline"), ellipse(5, 6, 15, 14, "body"), ellipse(5, 15, 15, 5, "shade"), ellipse(6, 7, 12, 8, "light"), op("noise", x=7, y=10, width=10, height=7, colors=["body", "light", "shade"], density=90), ellipse(7, 7, 4, 3, "shine"), box(8, 13, 2, 3, "eyes"), box(15, 13, 2, 3, "eyes"), box(10, 17, 5, 1, "eyes"), box(5, 19, 4, 1, "body"), box(17, 19, 3, 1, "body")]
        template = {"version": 2, "width": 24, "height": 24, "seed": 10, "palette": palette, "operations": operations}
        write(folder / "slime.template.json", template)
        project = folder / "variants.painter.json"
        self.run("project", "new", project, "--width", 24, "--height", 24)
        for index, seed in enumerate((10, 21, 42, 73, 99, 128)):
            canvas = folder / f"slime-{seed}.json"
            self.run("generate", "--input", folder / "slime.template.json", "--seed", seed, file=canvas)
            self.run("export", "--output", folder / f"slime-{seed}.png", file=canvas)
            if index:
                self.run("project", "frame", project, "add", f"seed-{seed}", "--duration-ms", 250)
            self.run("stamp", "--input", canvas, "--x", 0, "--y", 0, file=project)
        self.run("project", "tag", project, "set", "variants", "--frames", "0,1,2,3,4,5")
        self.finish(folder, project, columns=3, scale=8)

    def primitives(self):
        folder = self.root / "05-primitives-and-components"
        folder.mkdir()
        canvas = folder / "canvas.json"
        self.run("new", "--width", 48, "--height", 32, file=canvas)
        operations = [op("rect", x=1, y=1, width=14, height=14, color="#536287"), ellipse(3, 3, 10, 10, "#7196B9"), op("line", x1=-4, y1=18, x2=17, y2=29, color="#E5B875"), polygon([(20, 2), (29, 14), (17, 14)], "#D18A88"), op("copy", name="orb", x=1, y=1, width=14, height=14), op("paste", name="orb", x=32, y=1), op("erase", x=37, y=5, width=4, height=4), op("fill", x=37, y=5, color="#BEAED4"), box(20, 20, 8, 7, "#536287"), box(21, 21, 6, 5, "#E5B87580", over=True)]
        self.batch(folder, "shapes", operations, canvas)
        self.run("extract", "--x", 1, "--y", 1, "--width", 14, "--height", 14, "--output", folder / "orb.component.json", file=canvas)
        self.run("export", "--output", folder / "native.png", file=canvas)
        self.run("render", "--output", folder / "preview.png", "--scale", 8, file=canvas)
        imported = folder / "imported.json"
        self.run("import", "--input", folder / "native.png", file=imported)
        self.run("transform", "rotate", 1, file=imported)
        self.run("transform", "flip", "horizontal", file=imported)
        self.run("render", "--output", folder / "transformed-preview.png", "--scale", 8, file=imported)
        self.run("inspect", file=canvas, report=folder / "inspection.json")

    def overview(self):
        canvas = self.root / "overview.json"
        self.run("new", "--width", 160, "--height", 128, file=canvas)
        operations = [op("clear", color="#182535"), box(2, 2, 76, 62, "#283A47"), box(80, 2, 78, 62, "#283A47"), box(2, 66, 76, 60, "#283A47"), box(80, 66, 78, 60, "#283A47")]
        for source, x, y in [("01-crystal-keeper/native.png", 16, 10), ("02-moss-cottage/native.png", 87, 0), ("03-forest-tiles/native.png", 8, 80), ("04-seeded-slimes/export/sheet.png", 78, 68)]:
            temporary = self.root / (Path(source).parts[0] + ".component.json")
            self.run("import", "--input", self.root / source, file=temporary)
            operations.append(op("stamp", canvas=json.loads(temporary.read_text()), x=x, y=y, over=True))
        self.batch(self.root, "overview", operations, canvas)
        self.run("render", "--output", self.root / "overview.png", "--scale", 6, file=canvas)
        (self.root / "README.md").write_text('''# Pixel Painter review gallery

All pixels were drawn by `painter`; no image-generation service was used.

![Gallery overview](overview.png)

| Folder | Review output | What it exercises |
|---|---|---|
| [Crystal keeper](01-crystal-keeper/) | [Eight-frame preview](01-crystal-keeper/export/preview.png), [animated PNG](01-crystal-keeper/export/animation.png) | Four layers, four directions, two walk poses, pivots, tags, checkpoints |
| [Moss cottage](02-moss-cottage/) | [Enlarged preview](02-moss-cottage/export/preview.png) | Layered architecture, polygons, ellipse details, partial alpha |
| [Forest tiles](03-forest-tiles/) | [Tile atlas](03-forest-tiles/export/preview.png), [seam report](03-forest-tiles/seams.json) | Named palette, seeded texture, eight tiles, adjacency rules, padding |
| [Seeded slimes](04-seeded-slimes/) | [Six-variant preview](04-seeded-slimes/export/preview.png) | One reusable template, six seeds, components, atlas metadata |
| [Primitives](05-primitives-and-components/) | [Preview](05-primitives-and-components/preview.png), [transformed preview](05-primitives-and-components/transformed-preview.png) | Shapes, clipping, copy/paste, erase/fill, alpha, PNG import, rotate/flip |

Each asset folder contains editable JSON, operation/template inputs, a native PNG,
and an inspection report. Export bundles contain `sheet.png`, `preview.png`,
`frames/`, and `metadata.json`; temporal projects also contain `animation.png`.
The manifest is the completion marker. Coordinates and sizes are native pixels.

Regenerate into a new folder from the repository root:

```sh
cargo build --release --locked --manifest-path rust/Cargo.toml
python3 rust/examples/generate_gallery.py --painter rust/target/release/painter --output /tmp/painter-gallery
```

`SHA256SUMS` covers every gallery file except itself. Repeated runs with the same
binary produce identical bytes; inspect enlarged PNGs and APNGs in an image viewer.
''', encoding="utf-8")
        paths = sorted(p for p in self.root.rglob("*") if p.is_file())
        (self.root / "SHA256SUMS").write_text("".join(hashlib.sha256(p.read_bytes()).hexdigest() + "  " + p.relative_to(self.root).as_posix() + "\n" for p in paths), encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--painter", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    gallery = Gallery(args.painter, args.output)
    for method in (gallery.keeper, gallery.cottage, gallery.tiles, gallery.slimes, gallery.primitives, gallery.overview):
        method()
        print(method.__name__ + " complete", flush=True)
    print("Gallery: " + str(args.output.resolve()))


if __name__ == "__main__":
    main()
