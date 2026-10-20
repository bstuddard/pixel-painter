#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Create an independently authored public asset family using the actual CLI.

Python supplies recipes and orchestration; painter creates every raster pixel.
The destination must not exist. Requires only Python's standard library.
"""

import argparse
import hashlib
import json
from pathlib import Path

from generate_gallery import Gallery, box, ellipse, op, polygon, write

COLORS = {
    "outline": "#25323A",
    "top": "#E8D1A3",
    "front": "#C6AA7A",
    "side": "#937650",
    "highlight": "#F3E2BE",
    "shadow": "#18232B65",
    "roof": "#B96F5A",
    "roof_light": "#D79070",
    "roof_dark": "#804F48",
    "grass": "#6C8D58",
    "grass_light": "#89A768",
    "grass_dark": "#577648",
    "stone": "#9AA3A0",
    "stone_light": "#C2C9BB",
    "stone_dark": "#707E7D",
    "coat": "#4B9294",
    "coat_light": "#79B4AB",
    "coat_dark": "#386B77",
    "skin": "#D5AA80",
    "skin_light": "#E9C69B",
    "hair": "#695244",
    "boots": "#594B46",
    "window": "#AED0C6",
    "gold": "#DBC073",
}


def faces(top="top", front="front", side="side"):
    return {"top": top, "front": front, "side": side, "outline": "outline"}


def parameter(name):
    return {"param": name}


class Village(Gallery):
    def profile(self):
        profile = {
            "version": 1,
            "name": "Village Corner",
            "palette": {"version": 1, "colors": COLORS},
            "roles": {
                name: name
                for name in ("outline", "top", "front", "side", "highlight", "shadow")
            },
            "perspective": "three_quarter",
            "light": "northwest",
            "max_colors": 24,
            "width": {"min": 16, "max": 96},
            "height": {"min": 16, "max": 96},
            "animation": {
                "required_tags": [
                    f"walk-{d}" for d in ("south", "east", "north", "west")
                ],
                "margin": 1,
                "max_size_delta": 4,
            },
        }
        write(self.root / "profile.json", profile)
        write(self.root / "palette.json", profile["palette"])
        self.run(
            "style",
            "reference",
            "--profile",
            self.root / "profile.json",
            "--output",
            self.root / "style-reference.png",
        )

    def character(self):
        folder = self.root / "character"
        project = self.project(folder, 24, 32, ["body", "face"])
        for index, (direction, step) in enumerate(
            (d, s) for d in ("south", "east", "north", "west") for s in range(3)
        ):
            if index:
                self.run(
                    "project",
                    "frame",
                    project,
                    "add",
                    f"{direction}-{step}",
                    "--duration-ms",
                    160,
                )
            self.run(
                "project",
                "frame",
                project,
                "set",
                index,
                "--pivot-x",
                12,
                "--pivot-y",
                28,
            )
            self.layer_batch(
                folder,
                project,
                "base",
                [ellipse(5, 27, 16, 3, COLORS["shadow"])],
                f"{index:02}-shadow",
            )
            swing = (0, -1, 1)[step]
            body = [
                box(8, 24 + swing, 3, 4 - swing, COLORS["boots"]),
                box(14, 24 - swing, 3, 4 + swing, COLORS["boots"]),
                polygon([(7, 13), (16, 13), (18, 24), (6, 24)], COLORS["outline"]),
                polygon([(8, 14), (15, 14), (16, 23), (8, 23)], COLORS["coat"]),
                box(8, 14, 2, 8, COLORS["coat_light"]),
                box(15, 16, 2, 7, COLORS["coat_dark"]),
                box(6, 17 - swing, 2, 5, COLORS["skin"]),
                box(17, 17 + swing, 2, 5, COLORS["skin"]),
                box(8, 22, 8, 2, COLORS["boots"]),
                box(11, 22, 2, 2, COLORS["gold"]),
            ]
            self.layer_batch(folder, project, "body", body, f"{index:02}-body")
            head = [
                box(8, 5, 9, 9, COLORS["outline"]),
                box(9, 6, 7, 7, COLORS["skin"]),
                box(9, 6, 7, 3, COLORS["hair"]),
                box(9, 9, 2, 3, COLORS["skin_light"]),
            ]
            if direction == "north":
                head += [box(9, 8, 7, 4, COLORS["hair"])]
            elif direction == "south":
                head += [
                    box(10, 10, 1, 1, COLORS["outline"]),
                    box(14, 10, 1, 1, COLORS["outline"]),
                ]
            else:
                head += [
                    box(14 if direction == "east" else 10, 10, 1, 1, COLORS["outline"])
                ]
            self.layer_batch(folder, project, "face", head, f"{index:02}-face")
        for direction, base in zip(("south", "east", "north", "west"), range(0, 12, 3)):
            self.run(
                "project", "tag", project, "set", f"idle-{direction}", "--frames", base
            )
            self.run(
                "project",
                "tag",
                project,
                "set",
                f"walk-{direction}",
                "--frames",
                f"{base + 1},{base + 2}",
            )
        self.run("project", "frame", project, "select", 0)
        self.run(
            "animation",
            "check",
            "--input",
            project,
            "--profile",
            self.root / "profile.json",
            report=folder / "check.json",
        )
        self.run(
            "animation",
            "contact-sheet",
            "--input",
            project,
            "--output",
            folder / "contact-sheet.png",
            "--scale",
            6,
        )

    def tree(self):
        folder = self.root / "tree"
        project = self.project(folder, 48, 64, ["trunk", "canopy"])
        self.layer_batch(
            folder, project, "base", [ellipse(8, 55, 35, 6, COLORS["shadow"])], "shadow"
        )
        self.layer_batch(
            folder,
            project,
            "trunk",
            [
                polygon(
                    [(20, 22), (28, 22), (28, 52), (33, 58), (17, 58), (21, 50)],
                    COLORS["outline"],
                ),
                box(22, 24, 4, 31, COLORS["side"]),
                box(22, 28, 1, 24, COLORS["front"]),
            ],
            "trunk",
        )
        leaves = [
            ellipse(4, 6, 39, 34, COLORS["outline"]),
            ellipse(5, 7, 37, 31, COLORS["grass_dark"]),
            ellipse(7, 7, 31, 25, COLORS["grass"]),
            ellipse(9, 8, 21, 17, COLORS["grass_light"]),
            ellipse(4, 23, 19, 15, COLORS["grass"]),
            ellipse(29, 21, 13, 14, COLORS["grass_dark"]),
        ]
        self.layer_batch(folder, project, "canopy", leaves, "canopy")
        placement = {
            "version": 1,
            "anchors": {
                "ground": {"x": 24, "y": 58},
                "attachments": {"branch": {"x": 29, "y": 30}},
                "footprint": [
                    {"x": 18, "y": 54},
                    {"x": 31, "y": 54},
                    {"x": 31, "y": 59},
                    {"x": 18, "y": 59},
                ],
            },
            "parts": [
                {"id": "trunk", "layers": ["base", "trunk"], "order": 0},
                {"id": "canopy", "layers": ["canopy"], "order": 2},
            ],
        }
        write(folder / "placement.json", placement)

    def component(self, name, width, height, operations):
        folder = self.root / "components"
        folder.mkdir(exist_ok=True)
        canvas = folder / f"{name}.json"
        self.run("new", "--width", width, "--height", height, file=canvas)
        self.batch(folder, name, operations, canvas)
        return json.loads(canvas.read_text(encoding="utf-8"))

    def recipes(self):
        window = self.component(
            "window",
            9,
            10,
            [
                box(0, 0, 9, 10, COLORS["outline"]),
                box(1, 1, 7, 8, COLORS["window"]),
                box(4, 1, 1, 8, COLORS["side"]),
                box(1, 4, 7, 1, COLORS["side"]),
                box(0, 9, 9, 1, COLORS["top"]),
            ],
        )
        door = self.component(
            "door",
            10,
            18,
            [
                box(0, 0, 10, 18, COLORS["outline"]),
                box(1, 1, 8, 16, COLORS["side"]),
                box(2, 2, 1, 14, COLORS["front"]),
                box(7, 9, 1, 1, COLORS["gold"]),
            ],
        )
        cottage = {
            "version": 2,
            "width": 96,
            "height": 96,
            "parameters": {
                "width": {"type": "integer", "default": 42, "min": 32, "max": 62},
                "height": {"type": "integer", "default": 28, "min": 24, "max": 36},
                "depth": {"type": "integer", "default": 8, "min": 4, "max": 12},
                "window_y": {"type": "integer", "default": 49, "min": 40, "max": 54},
                "second_window_x": {
                    "type": "integer",
                    "default": 42,
                    "min": 38,
                    "max": 62,
                },
                "roof_color": {"type": "color", "default": "roof_light"},
            },
            "variants": {
                "workshop": {
                    "width": 60,
                    "height": 36,
                    "depth": 10,
                    "window_y": 42,
                    "second_window_x": 60,
                    "roof_color": "coat",
                }
            },
            "parts": {"window": window, "door": door},
            "operations": [
                ellipse(10, 69, 66, 8, "shadow"),
                {
                    "shape": "cottage",
                    "x": 12,
                    "y": 72,
                    "width": parameter("width"),
                    "height": parameter("height"),
                    "depth": parameter("depth"),
                    "roof_height": 12,
                    "faces": faces(),
                    "roof": faces(parameter("roof_color"), "roof", "roof_dark"),
                },
                {
                    "op": "component",
                    "name": "window",
                    "x": 18,
                    "y": parameter("window_y"),
                    "over": True,
                },
                {
                    "op": "component",
                    "name": "window",
                    "x": parameter("second_window_x"),
                    "y": parameter("window_y"),
                    "flip_x": True,
                    "over": True,
                },
                {"op": "component", "name": "door", "x": 30, "y": 55, "over": True},
            ],
        }
        write(self.root / "recipes/cottage.json", cottage)
        recipes = {
            "crate": (
                32,
                32,
                [
                    ellipse(5, 25, 24, 4, "shadow"),
                    {
                        "shape": "prism",
                        "x": 5,
                        "y": 12,
                        "width": 18,
                        "height": 14,
                        "depth": 5,
                        "faces": faces(),
                    },
                    box(6, 17, 16, 1, "side"),
                    box(6, 22, 16, 1, "side"),
                    op("line", x1=7, y1=13, x2=20, y2=24, color="highlight"),
                ],
            ),
            "bench": (
                48,
                40,
                [
                    ellipse(5, 31, 39, 5, "shadow"),
                    box(9, 24, 3, 10, "outline"),
                    box(35, 24, 3, 10, "outline"),
                    {
                        "shape": "prism",
                        "x": 6,
                        "y": 22,
                        "width": 32,
                        "height": 4,
                        "depth": 5,
                        "faces": faces(),
                    },
                    box(8, 11, 3, 12, "side"),
                    box(34, 11, 3, 12, "side"),
                    box(7, 10, 30, 5, "outline"),
                    box(8, 11, 28, 3, "front"),
                ],
            ),
            "lamp": (
                32,
                64,
                [
                    ellipse(6, 55, 23, 5, "shadow"),
                    box(14, 16, 4, 42, "outline"),
                    box(15, 18, 1, 38, "stone"),
                    box(11, 55, 10, 3, "outline"),
                    polygon([(9, 8), (16, 4), (23, 8)], "outline"),
                    box(10, 9, 13, 10, "outline"),
                    box(12, 10, 9, 7, "gold"),
                    box(16, 10, 1, 7, "side"),
                ],
            ),
            "curb": (
                32,
                32,
                [
                    {
                        "shape": "prism",
                        "x": 2,
                        "y": 18,
                        "width": 24,
                        "height": 5,
                        "depth": 4,
                        "faces": faces("stone_light", "stone", "stone_dark"),
                    }
                ],
            ),
            "stairs": (
                32,
                32,
                [
                    {
                        "shape": "steps",
                        "x": 2,
                        "y": 24,
                        "width": 18,
                        "height": 4,
                        "depth": 3,
                        "count": 3,
                        "faces": faces("stone_light", "stone", "stone_dark"),
                    }
                ],
            ),
        }
        for name, (width, height, operations) in recipes.items():
            write(
                self.root / f"recipes/{name}.json",
                {
                    "version": 2,
                    "width": width,
                    "height": height,
                    "operations": operations,
                },
            )
        for name, base, accent in [
            ("grass", "grass", "grass_light"),
            ("path", "front", "top"),
            ("pavement", "stone", "stone_light"),
        ]:
            operations = [op("clear", color=base)]
            # Edges stay uniform for exact seams; all interior detail is deterministic.
            for x, y in [(2, 3), (7, 10), (11, 5), (4, 12), (12, 12)]:
                operations.append(box(x, y, 2, 1, accent))
            write(
                self.root / f"recipes/{name}.json",
                {"version": 2, "width": 16, "height": 16, "operations": operations},
            )

    def pack_and_scene(self):
        assets = [
            {
                "id": "adventurer",
                "source": {
                    "kind": "artwork",
                    "input": "character/project.painter.json",
                },
            },
            {
                "id": "tree",
                "source": {"kind": "artwork", "input": "tree/project.painter.json"},
                "placement": "tree/placement.json",
            },
        ]
        for name in (
            "cottage",
            "workshop",
            "crate",
            "bench",
            "lamp",
            "curb",
            "stairs",
            "grass",
            "path",
            "pavement",
        ):
            source = {
                "kind": "recipe",
                "input": f"recipes/{'cottage' if name == 'workshop' else name}.json",
            }
            if name == "workshop":
                source["variant"] = "workshop"
            assets.append({"id": name, "source": source})
        write(
            self.root / "pack.recipe.json",
            {"version": 1, "seed": 47, "profile": "profile.json", "assets": assets},
        )
        self.run(
            "pack",
            "generate",
            "--input",
            self.root / "pack.recipe.json",
            "--output",
            self.root / "export",
            report=self.root / "generation-check.json",
        )
        self.run(
            "pack",
            "check",
            "--input",
            self.root / "export",
            report=self.root / "pack-check.json",
        )
        ids = {asset["id"]: index for index, asset in enumerate(assets)}

        def entry(name, x, y, *, label=None, anchor=None, order=0, part=None):
            result = {
                "id": label or name,
                "input": f"export/assets/{ids[name]:04}/export/frames/0000.png",
                "position": {"x": x, "y": y},
                "order": order,
            }
            if anchor:
                result["anchor"] = {"x": anchor[0], "y": anchor[1]}
            if part:
                result.update(
                    placement=f"export/assets/{ids[name]:04}/export/placement.json",
                    part=part,
                )
            return result

        entries = []
        for row in range(6, 9):
            for column in range(1, 15):
                entries.append(
                    entry(
                        "path",
                        column * 16,
                        row * 16,
                        label=f"path-{row}-{column}",
                        anchor=(0, 0),
                        order=-10,
                    )
                )
        for column in range(3, 8):
            entries.append(
                entry(
                    "pavement",
                    column * 16,
                    80,
                    label=f"paving-{column}",
                    anchor=(0, 0),
                    order=-10,
                )
            )
        entries += [
            entry("cottage", 35, 92, anchor=(12, 72)),
            entry("workshop", 141, 90, anchor=(12, 72)),
            entry("bench", 119, 149, anchor=(24, 34)),
            entry("crate", 203, 142, anchor=(16, 26)),
            entry("lamp", 24, 149, anchor=(16, 58)),
            entry("stairs", 128, 96, anchor=(16, 28)),
            entry("curb", 64, 96, anchor=(16, 24)),
            entry("tree", 225, 100, label="tree-trunk", part="trunk"),
            entry("adventurer", 214, 101, anchor=(12, 28)),
            entry("tree", 225, 100, label="tree-canopy", part="canopy"),
        ]
        scene = {
            "version": 1,
            "width": 256,
            "height": 176,
            "y_sort": True,
            "tile": f"export/assets/{ids['grass']:04}/export/frames/0000.png",
            "assets": entries,
        }
        write(self.root / "scene.json", scene)
        self.run(
            "scene",
            "render",
            "--input",
            self.root / "scene.json",
            "--output",
            self.root / "scene.png",
            "--scale",
            4,
        )
        options = {
            "width": 64,
            "height": 32,
            "origin": {"x": 1, "y": 2},
            "step": {"x": 1, "y": 1},
            "steps": 5,
            "squash_y": 4,
            "color": COLORS["shadow"],
        }
        write(self.root / "shadow.options.json", options)
        self.run(
            "shadow",
            "--input",
            self.root / f"export/assets/{ids['crate']:04}/export/frames/0000.png",
            "--options",
            self.root / "shadow.options.json",
            "--output",
            self.root / "projected-shadow.png",
        )

    def finish_gallery(self):
        (self.root / "README.md").write_text(
            """# Village Corner

![Offline scene rendered by painter](scene.png)

Review `scene.png` for shared scale, light, perspective, and overlap;
`export/contact-sheet.png` for individual assets; `character/contact-sheet.png`
for four-direction idle/walk frames; and `style-reference.png` for palette roles.
The character stands between the exported tree trunk and canopy.

`pack-check.json`, `generation-check.json`, and `character/check.json` contain
measurable validation results. They do not grade artistic quality.
`projected-shadow.png` demonstrates the separate bounded silhouette projector.

All artwork was independently authored for this public toolkit. Every raster
pixel was produced by painter CLI operations, without external artwork or image
generation. These samples are GPL-3.0-only, like the repository; using painter
does not impose this license on independently authored user artwork.

Edit `profile.json`, `recipes/`, the character/tree projects, or `scene.json`.
`pack.recipe.json` assembles the family; `export/pack.json` indexes the results.
Tree placement contains ground/attachment/footprint metadata and split layers.
Cottage and workshop share one parameterized recipe with reusable windows/door.
Curbs and stairs use the same bounded face helpers as buildings and props.

Reproduce into a new folder from the repository root:

```sh
cargo build --release --locked --manifest-path rust/Cargo.toml
python3 rust/examples/generate_village.py --painter rust/target/release/painter --output /tmp/painter-village
diff rust/examples/village/SHA256SUMS /tmp/painter-village/SHA256SUMS
```

Paths inside JSON are relative to their configuration directory. No private
downstream project, local machine path, or external service is required.
""",
            encoding="utf-8",
        )
        paths = sorted(p for p in self.root.rglob("*") if p.is_file())
        (self.root / "SHA256SUMS").write_text(
            "".join(
                f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(self.root).as_posix()}\n"
                for p in paths
            ),
            encoding="utf-8",
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--painter", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    village = Village(args.painter, args.output)
    village.profile()
    village.character()
    village.tree()
    village.recipes()
    village.pack_and_scene()
    village.finish_gallery()


if __name__ == "__main__":
    main()
