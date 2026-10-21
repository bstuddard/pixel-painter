#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Reproduce v0.4 authoring and review examples using the actual painter CLI.

Only Python's standard library is needed. The output must not exist. All raster
edits, previews, and compositions are performed by painter, including the hero.
"""

import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def rect(x, y, width, height, color):
    return dict(op="fill_rect", x=x, y=y, width=width, height=height, color=color)


def polygon(points, color):
    return dict(
        op="polygon", points=[dict(x=x, y=y) for x, y in points], color=color, fill=True
    )


def ellipse(x, y, width, height, color):
    return dict(
        op="ellipse", x=x, y=y, width=width, height=height, color=color, fill=True
    )


class Workshop:
    def __init__(self, painter, root):
        self.painter = str(painter.resolve())
        self.root = root
        root.mkdir(parents=True, exist_ok=False)

    def run(self, *args, file=None):
        command = [self.painter, "--json"]
        if file:
            command += ["--file", str(file)]
        result = subprocess.run(
            command + list(map(str, args)), capture_output=True, text=True
        )
        if result.returncode:
            raise RuntimeError(result.stderr)
        return json.loads(result.stdout)

    def batch(self, folder, name, project, operations, **options):
        path = folder / (name + ".ops.json")
        write(path, dict(version=1, operations=operations, **options))
        self.run("batch", "--input", path, file=project)

    def layer(self, project, name, channel="color"):
        self.run("project", "layer", project, "add", name, "--channel", channel)

    def select(self, project, name):
        self.run("project", "layer", project, "select", name)

    def finish(self, folder, project, columns):
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
            columns,
            "--scale",
            4,
        )
        self.run(
            "preview",
            "--output",
            folder / "review",
            "--all-frames",
            "--scale",
            3,
            file=project,
        )

    def chest(self):
        folder = self.root / "01-material-chest"
        folder.mkdir()
        project = folder / "project.painter.json"
        self.run("project", "new", project, "--width", 64, "--height", 56)
        self.layer(project, "normals", "normal")
        self.select(project, "base")
        shape = dict(
            shape="prism",
            x=9,
            y=20,
            width=40,
            height=25,
            depth=7,
            faces=dict(
                top="#D99460", front="#A85F46", side="#623C40", outline="#202A3C"
            ),
        )
        write(folder / "geometry.json", shape)
        self.run(
            "project",
            "geometry",
            project,
            "--input",
            folder / "geometry.json",
            "--normal-layer",
            "normals",
        )
        details = [rect(11, 30, 37, 2, "#623C40"), rect(11, 32, 37, 1, "#D99460")]
        for x in (15, 38):
            details += [
                rect(x, 21, 5, 23, "#3E5169"),
                rect(x + 1, 21, 3, 22, "#728DA0"),
                rect(x + 1, 22, 1, 21, "#B6CFCD"),
                polygon([(x, 20), (x + 7, 13), (x + 11, 13), (x + 4, 20)], "#B6CFCD"),
            ]
            for y in (24, 39):
                details += [rect(x + 2, y, 1, 1, "#202A3C")]
        details += [
            rect(27, 28, 8, 10, "#202A3C"),
            rect(28, 29, 6, 8, "#B6CFCD"),
            polygon([(31, 29), (34, 32), (31, 35), (28, 32)], "#51B7B0"),
            rect(31, 30, 1, 2, "#B7ECD4"),
        ]
        for x, y in [(23, 25), (25, 40), (42, 34), (11, 36)]:
            details += [rect(x, y, 3, 1, "#D99460")]
        self.batch(folder, "wood-and-steel", project, details)
        self.layer(project, "glow", "emissive")
        self.batch(
            folder,
            "rune-glow",
            project,
            [polygon([(31, 29), (34, 32), (31, 35), (28, 32)], "#51B7B080")],
        )
        self.select(project, "base")
        ramps = [
            dict(name="wood", colors=["#623C40", "#A85F46", "#D99460", "#F2C280"]),
            dict(name="steel", colors=["#3E5169", "#728DA0", "#B6CFCD", "#E4ECD8"]),
        ]
        write(folder / "materials.json", ramps)
        for name, steps in [("shadow", -1), ("highlight", 1)]:
            self.run(
                "project",
                "frame",
                project,
                "add",
                name,
                "--duplicate",
                0,
                "--duration-ms",
                450,
            )
            self.batch(
                folder, name, project, [dict(op="shade", ramps=ramps, steps=steps)]
            )
        self.finish(folder, project, 3)

    def sentinel(self):
        folder = self.root / "02-masked-sentinel"
        folder.mkdir()
        project = folder / "project.painter.json"
        self.run("project", "new", project, "--width", 48, "--height", 64)
        body = [
            ellipse(9, 56, 30, 5, "#0D192980"),
            rect(15, 49, 7, 9, "#293746"),
            rect(27, 49, 7, 9, "#293746"),
            rect(13, 56, 10, 3, "#162536"),
            rect(26, 56, 10, 3, "#162536"),
            polygon([(12, 27), (36, 27), (41, 49), (32, 54), (9, 49)], "#162536"),
            polygon([(14, 29), (34, 29), (38, 47), (31, 51), (12, 47)], "#3A777C"),
            polygon([(14, 29), (20, 29), (17, 47), (12, 47)], "#5C9C91"),
            polygon([(30, 29), (34, 29), (38, 47), (31, 51)], "#295260"),
            rect(7, 30, 6, 14, "#162536"),
            rect(8, 31, 4, 10, "#728D8C"),
            rect(37, 31, 6, 14, "#162536"),
            rect(38, 32, 4, 10, "#728D8C"),
            ellipse(10, 9, 29, 24, "#162536"),
            ellipse(12, 10, 25, 20, "#728D8C"),
            polygon([(12, 16), (18, 11), (31, 11), (36, 16)], "#B4C6AA"),
            rect(14, 18, 21, 8, "#293746"),
            rect(16, 20, 5, 3, "#B7ECD4"),
            rect(28, 20, 5, 3, "#B7ECD4"),
            rect(23, 25, 4, 1, "#B4C6AA"),
            rect(23, 5, 3, 6, "#728D8C"),
            polygon([(24, 7), (21, 2), (16, 1), (19, 6)], "#82B66C"),
            polygon([(25, 6), (29, 1), (34, 2), (29, 6)], "#B4D77C"),
        ]
        self.batch(folder, "sentinel", project, body)
        mask = folder / "cloak-mask.json"
        self.run("new", "--width", 48, "--height", 64, file=mask)
        self.batch(
            folder,
            "selection",
            mask,
            [polygon([(15, 31), (33, 31), (36, 46), (30, 49), (14, 46)], "#FFFFFF")],
        )
        self.run("export", "--output", folder / "cloak-mask.png", file=mask)
        self.layer(project, "normals", "normal")
        self.batch(folder, "flat-normals", project, [rect(0, 0, 48, 64, "#8080FF")])
        self.layer(project, "glow", "emissive")
        self.batch(
            folder,
            "eyes",
            project,
            [rect(16, 20, 5, 3, "#B7ECD480"), rect(28, 20, 5, 3, "#B7ECD480")],
        )
        self.select(project, "base")
        self.run(
            "project",
            "frame",
            project,
            "add",
            "moss-brush",
            "--duplicate",
            0,
            "--duration-ms",
            500,
        )
        brush = folder / "moss-brush.json"
        self.run("new", "--width", 8, "--height", 8, file=brush)
        self.batch(
            folder,
            "brush",
            brush,
            [
                rect(1, 2, 3, 2, "#82B66C"),
                rect(4, 5, 3, 1, "#B4D77C"),
                rect(2, 4, 2, 1, "#518369"),
            ],
        )
        for x, y in [(10, 31), (18, 34), (28, 36), (32, 44), (10, 45)]:
            self.run(
                "stamp",
                "--input",
                brush,
                "--x",
                x,
                "--y",
                y,
                "--mask",
                mask,
                "--alpha-lock",
                "--over",
                file=project,
            )
        self.run(
            "draw",
            "--mask",
            folder / "cloak-mask.png",
            "--alpha-lock",
            "line",
            "--x1",
            4,
            "--y1",
            38,
            "--x2",
            44,
            "--y2",
            47,
            "--color",
            "#B4D77C",
            file=project,
        )
        self.run(
            "render", "--output", folder / "mask-preview.png", "--scale", 4, file=mask
        )
        self.finish(folder, project, 2)

    def cottage(self):
        folder = self.root / "03-cottage-review"
        folder.mkdir()
        project = folder / "project.painter.json"
        source = (
            Path(__file__).parent / "lighting/02-night-cottage/project.painter.json"
        )
        shutil.copyfile(source, project)
        self.run(
            "preview",
            "--output",
            folder / "review",
            "--all-frames",
            "--scale",
            3,
            file=project,
        )
        for name, layer, raw in [
            ("color-solo", "base", False),
            ("raw-normals", "normals", True),
        ]:
            args = [
                "render",
                "--solo-layer",
                layer,
                "--output",
                folder / (name + ".png"),
                "--scale",
                4,
            ]
            if raw:
                args.append("--raw")
            self.run(*args, file=project)

    def hero(self):
        scene = dict(version=1, width=256, height=132, background="#101D2B", assets=[])
        items = [
            ("01-material-chest/export/frames/0000.png", 0, 0),
            ("01-material-chest/export/frames/0001.png", 64, 0),
            ("01-material-chest/export/frames/0002.png", 128, 0),
            ("02-masked-sentinel/export/frames/0001.png", 196, 0),
            ("03-cottage-review/review/frames/0000/light-left.png", 4, 65),
            ("03-cottage-review/review/frames/0000/light-right.png", 70, 65),
            ("03-cottage-review/review/frames/0000/light-top.png", 136, 65),
            ("02-masked-sentinel/export/frames/0000.png", 196, 64),
        ]
        for index, (path, x, y) in enumerate(items):
            scene["assets"].append(
                dict(
                    id=f"asset-{index}",
                    input=path,
                    position=dict(x=x, y=y),
                    anchor=dict(x=0, y=0),
                )
            )
        write(self.root / "hero.scene.json", scene)
        self.run(
            "scene",
            "render",
            "--input",
            self.root / "hero.scene.json",
            "--output",
            self.root / "hero.png",
            "--scale",
            4,
        )

    def generate(self):
        self.chest()
        self.sentinel()
        self.cottage()
        self.hero()
        lines = []
        for path in sorted(self.root.rglob("*")):
            if path.is_file():
                lines.append(
                    f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(self.root).as_posix()}"
                )
        (self.root / "SHA256SUMS").write_text("\n".join(lines) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--painter", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    Workshop(args.painter, args.output).generate()


if __name__ == "__main__":
    main()
