#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Reproduce editable map examples and offline lighting references with painter.

The destination must not exist. Authoring/export runs through the CLI. The small
CPU lighting reference is example code, separate from the offline Rust tool.
"""

import argparse
import hashlib
import json
import math
import subprocess
import tempfile
from pathlib import Path


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def rect(x, y, width, height, color):
    return dict(op="fill_rect", x=x, y=y, width=width, height=height, color=color)


def normal(x, y, z):
    length = math.sqrt(x * x + y * y + z * z)
    return "#" + "".join(
        f"{math.floor((v / length + 1) * 127.5 + 0.5):02X}" for v in (x, y, z)
    )


def linear(value):
    value /= 255
    return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4


def srgb(value):
    value = min(1, max(0, value))
    value = value * 12.92 if value <= 0.0031308 else 1.055 * value ** (1 / 2.4) - 0.055
    return math.floor(value * 255 + 0.5)


class Examples:
    def __init__(self, painter, root):
        self.painter = str(painter.resolve())
        self.root = root
        root.mkdir(parents=True, exist_ok=False)

    def run(self, *args, file=None):
        command = [self.painter, "--json"]
        if file:
            command += ["--file", str(file)]
        result = subprocess.run(
            command + list(map(str, args)), capture_output=True, text=True, check=True
        )
        return json.loads(result.stdout)

    def batch(self, folder, name, project, operations):
        path = folder / (name + ".ops.json")
        write(path, dict(version=1, operations=operations))
        self.run("batch", "--input", path, file=project)

    def project(self, folder, width, height):
        folder.mkdir()
        project = folder / "project.painter.json"
        self.run("project", "new", project, "--width", width, "--height", height)
        self.run("project", "layer", project, "add", "normals", "--channel", "normal")
        self.run("project", "layer", project, "select", "base")
        return project

    def geometry(self, folder, project, shape):
        path = folder / "geometry.json"
        write(path, shape)
        self.run(
            "project", "geometry", project, "--input", path, "--normal-layer", "normals"
        )

    def finish(self, folder, project):
        self.run("project", "frame", project, "select", 0)
        self.run(
            "project",
            "export",
            project,
            "--output",
            folder / "export",
            "--maps",
            "emissive,normal",
            "--columns",
            2 if folder.name == "01-neon-sign" else 1,
            "--padding",
            2,
            "--margin",
            1,
            "--scale",
            4,
        )
        self.run(
            "project",
            "export",
            project,
            "--output",
            folder / "color-only",
            "--columns",
            2 if folder.name == "01-neon-sign" else 1,
            "--padding",
            2,
            "--margin",
            1,
            "--scale",
            4,
        )
        return self.review(folder)

    def sign(self):
        folder = self.root / "01-neon-sign"
        project = self.project(folder, 32, 32)
        self.geometry(
            folder,
            project,
            dict(
                shape="prism",
                x=3,
                y=7,
                width=24,
                height=18,
                depth=3,
                faces=dict(
                    top="#607E96", front="#263A50", side="#172838", outline="#0D1826"
                ),
            ),
        )
        letters = []
        for offset, glyph in [
            (6, ["111", "101", "111", "100", "100"]),
            (12, ["111", "010", "010", "010", "111"]),
            (18, ["101", "101", "010", "101", "101"]),
        ]:
            for y, row in enumerate(glyph):
                for x, value in enumerate(row):
                    if value == "1":
                        letters.append(rect(offset + x, 12 + y * 2, 1, 2, "#EC6B97"))
        self.batch(folder, "letters", project, letters)
        self.run("project", "layer", project, "add", "glow", "--channel", "emissive")
        self.batch(
            folder,
            "emission-dim",
            project,
            [dict(item, color="#EC6B9780") for item in letters],
        )
        self.run("project", "frame", project, "set", 0, "--duration-ms", 170)
        self.run(
            "project",
            "frame",
            project,
            "add",
            "bright",
            "--duplicate",
            0,
            "--duration-ms",
            230,
        )
        self.batch(folder, "emission-bright", project, letters)
        self.run("project", "tag", project, "set", "pulse", "--frames", "0,1")
        return self.finish(folder, project)

    def cottage(self):
        folder = self.root / "02-night-cottage"
        project = self.project(folder, 64, 64)
        self.geometry(
            folder,
            project,
            dict(
                shape="cottage",
                x=9,
                y=52,
                width=39,
                height=26,
                depth=8,
                roof_height=16,
                faces=dict(
                    top="#D6BFA0", front="#A78872", side="#6C676B", outline="#1D2739"
                ),
                roof=dict(
                    top="#5C7595", front="#405976", side="#2D3D58", outline="#182336"
                ),
            ),
        )
        windows = [rect(15, 34, 8, 10, "#F2AC65"), rect(34, 34, 8, 10, "#F2AC65")]
        self.batch(
            folder,
            "details",
            project,
            [rect(25, 37, 7, 16, "#332F3F"), rect(26, 39, 1, 12, "#8D6970")]
            + windows
            + [
                rect(18, 34, 1, 10, "#594D58"),
                rect(37, 34, 1, 10, "#594D58"),
                rect(15, 39, 8, 1, "#594D58"),
                rect(34, 39, 8, 1, "#594D58"),
            ],
        )
        self.run("project", "layer", project, "add", "windows", "--channel", "emissive")
        self.batch(
            folder,
            "window-emission",
            project,
            [dict(item, color="#FFC875C0") for item in windows]
            + [
                rect(18, 34, 1, 10, "#00000000"),
                rect(37, 34, 1, 10, "#00000000"),
                rect(15, 39, 8, 1, "#00000000"),
                rect(34, 39, 8, 1, "#00000000"),
            ],
        )
        return self.finish(folder, project)

    def transforms(self):
        folder = self.root / "03-vector-transforms"
        project = self.project(folder, 16, 16)
        self.batch(
            folder,
            "color",
            project,
            [
                rect(2, 2, 11, 11, "#4F9B9A"),
                rect(2, 2, 5, 5, "#97D8C2"),
                rect(9, 10, 4, 3, "#355674"),
            ],
        )
        self.run("project", "layer", project, "select", "normals")
        self.batch(
            folder,
            "authored-normals",
            project,
            [
                rect(2, 2, 11, 11, normal(0, 0, 1)),
                rect(2, 2, 5, 5, normal(-1, 1, 1)),
                rect(9, 10, 4, 3, normal(1, -1, 1)),
            ],
        )
        for name, transform in [
            ("flip-x", ["flip", "horizontal"]),
            ("turn-cw", ["rotate", 1]),
        ]:
            self.run("project", "frame", project, "add", name, "--duplicate", 0)
            for layer in ["base", "normals"]:
                self.run("project", "layer", project, "select", layer)
                self.run("transform", *transform, file=project)
        return self.finish(folder, project)

    def read_png(self, path, temporary):
        target = temporary / f"image-{len(list(temporary.iterdir()))}.json"
        self.run("import", "--input", path, file=target)
        return self.run("print", file=target)

    def export_canvas(self, canvas, output):
        with tempfile.TemporaryDirectory(prefix="painter-lighting-") as directory:
            source = Path(directory) / "canvas.json"
            write(source, canvas)
            self.run("render", "--output", output, "--scale", 4, file=source)

    def review(self, folder):
        with tempfile.TemporaryDirectory(prefix="painter-lighting-") as directory:
            temporary = Path(directory)
            color = self.read_png(folder / "export/frames/0000.png", temporary)
            emissive = self.read_png(
                folder / "export/emissive/frames/0000.png", temporary
            )
            normals = self.read_png(folder / "export/normal/frames/0000.png", temporary)
        panels = []
        lights = [None, None, (-1, 0.3, 0.6), (1, 0.3, 0.6), (0, 1, 0.5)]
        for mode, light in enumerate(lights):
            if light:
                length = math.sqrt(sum(value * value for value in light))
                light = [value / length for value in light]
            pixels = []
            for albedo, emission, surface in zip(
                color["pixels"], emissive["pixels"], normals["pixels"]
            ):
                diffuse = 1
                if light:
                    direction = [surface[key] / 127.5 - 1 for key in "rgb"]
                    length = math.sqrt(sum(value * value for value in direction))
                    diffuse = 0.18 + 0.82 * max(
                        0,
                        sum(
                            value / length * incoming
                            for value, incoming in zip(direction, light)
                        ),
                    )
                strength = emission["a"] / albedo["a"] if albedo["a"] else 0
                rgb = [
                    srgb(
                        linear(albedo[key]) * diffuse
                        + (linear(emission[key]) * strength if mode else 0)
                    )
                    for key in "rgb"
                ]
                alpha = albedo["a"] / 255
                pixels.append(
                    dict(
                        zip(
                            "rgba",
                            [
                                math.floor(
                                    value * alpha + background * (1 - alpha) + 0.5
                                )
                                for value, background in zip(rgb, [24, 37, 53])
                            ]
                            + [255],
                        )
                    )
                )
            panels.append(pixels)
        width, height = color["width"], color["height"]
        output = dict(width=width * 5, height=height, pixels=[])
        for y in range(height):
            for panel in panels:
                output["pixels"].extend(panel[y * width : (y + 1) * width])
        self.export_canvas(output, folder / "review.png")
        return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--painter", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    examples = Examples(args.painter, args.output)
    panels = [examples.sign(), examples.cottage(), examples.transforms()]
    width = max(panel["width"] for panel in panels)
    overview = dict(
        width=width, height=sum(panel["height"] for panel in panels), pixels=[]
    )
    background = dict(r=24, g=37, b=53, a=255)
    for panel in panels:
        for y in range(panel["height"]):
            overview["pixels"].extend(
                panel["pixels"][y * panel["width"] : (y + 1) * panel["width"]]
            )
            overview["pixels"].extend([background] * (width - panel["width"]))
    examples.export_canvas(overview, args.output / "overview.png")
    sums = []
    for path in sorted(args.output.rglob("*")):
        if path.is_file():
            sums.append(
                f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(args.output).as_posix()}"
            )
    (args.output / "SHA256SUMS").write_text("\n".join(sums) + "\n", encoding="utf-8")
    print(f"Generated {len(sums)} reproducible lighting artifacts")


if __name__ == "__main__":
    main()
