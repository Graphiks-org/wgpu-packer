import json
import pathlib


def load() -> dict:
    p = pathlib.Path(__file__).resolve().parent.parent / "targets" / "matrix.json"
    return json.loads(p.read_text())


def by_kotlin_target(name: str) -> dict:
    return next(t for t in load()["targets"] if t["kotlinTarget"] == name)


def by_triple(triple: str) -> dict:
    return next(t for t in load()["targets"] if t["triple"] == triple)
