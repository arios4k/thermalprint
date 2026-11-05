#!/usr/bin/env python3
"""Generate an offline-buildable RPM source archive and spec; requires Python 3.11+."""
import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib


def prepare(project: Path, output: Path, offline: bool = False) -> Path:
    manifest = tomllib.loads((project / "Cargo.toml").read_text())
    version = manifest["package"]["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("RPM releases require a numeric major.minor.patch version")
    output.mkdir(parents=True, exist_ok=True)
    for directory in ("SOURCES", "SPECS", "SRPMS", "RPMS", "BUILD", "BUILDROOT"):
        (output / directory).mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="thermalprint-rpm-") as temporary:
        stage = Path(temporary) / f"thermalprint-{version}"
        stage.mkdir()
        for name in ("Cargo.toml", "Cargo.lock", "LICENSE", "README.md", "src", "data", "assets", "tests"):
            source = project / name
            if source.is_dir():
                shutil.copytree(source, stage / name)
            else:
                shutil.copy2(source, stage / name)
        command = ["cargo", "vendor", "--locked", "--versioned-dirs"]
        if offline:
            command.append("--offline")
        command.append("vendor")
        result = subprocess.run(command, cwd=stage, check=True, text=True, stdout=subprocess.PIPE)
        (stage / ".cargo").mkdir()
        (stage / ".cargo/config.toml").write_text(result.stdout)
        licenses = {manifest["package"]["license"], "Bitstream-Vera"}
        provides, inventory = [], []
        for crate in sorted((stage / "vendor").iterdir()):
            metadata = tomllib.loads((crate / "Cargo.toml").read_text())["package"]
            name, crate_version = metadata["name"], metadata["version"]
            license_expression = metadata.get("license", "").strip()
            if not license_expression:
                raise ValueError(f"{name} {crate_version} has no license expression; review before distributing")
            # Older manifests use a slash to mean dual licensing.
            license_expression = re.sub(r"\s*/\s*", " OR ", license_expression)
            licenses.add(license_expression)
            provides.append(f"Provides:       bundled(crate({name})) = {crate_version}")
            inventory.append(f"{name} {crate_version}: {license_expression}")
            destination = stage / "vendor-licenses" / crate.name
            for entry in sorted(crate.rglob("*")):
                if entry.is_file() and any(word in entry.name.lower() for word in ("license", "licence", "copyright", "notice", "copying")):
                    target = destination / entry.relative_to(crate)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(entry, target)
        (stage / "VENDORED-LICENSES.txt").write_text(
            "Locked Rust dependency license inventory (all target platforms).\n\n" + "\n".join(inventory) + "\n"
        )
        # Include all declared alternatives conservatively in the RPM license expression.
        expression = " AND ".join(f"({value})" if " " in value else value for value in sorted(licenses))
        spec = (project / "packaging/rpm/thermalprint.spec.in").read_text()
        spec = spec.replace("@VERSION@", version).replace("@LICENSE@", expression)
        spec = spec.replace("@BUNDLED_PROVIDES@", "\n".join(provides))
        spec_path = output / "SPECS/thermalprint.spec"
        spec_path.write_text(spec)
        archive = output / "SOURCES" / f"thermalprint-{version}.tar.xz"
        epoch = int(os.environ.get("SOURCE_DATE_EPOCH") or "0")
        def normalize(info):
            info.uid = info.gid = 0
            info.uname = info.gname = "root"
            info.mtime = epoch
            return info
        with tarfile.open(archive, "w:xz", preset=3) as tar:
            tar.add(stage, arcname=stage.name, filter=normalize)
        print(f"Prepared {archive} and {spec_path}")
    return spec_path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("target/rpm"))
    parser.add_argument("--offline", action="store_true", help="Use only crates already cached locally")
    parser.add_argument("--srpm", action="store_true", help="Also invoke rpmbuild -bs")
    args = parser.parse_args()
    project = Path(__file__).resolve().parent.parent
    output = args.output.resolve()
    spec = prepare(project, output, args.offline)
    if args.srpm:
        subprocess.run(["rpmbuild", "-bs", "--define", f"_topdir {output}", str(spec)], check=True)


if __name__ == "__main__":
    main()
