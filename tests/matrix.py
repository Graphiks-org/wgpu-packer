import json
import pathlib


def load() -> dict:
    p = pathlib.Path(__file__).resolve().parent.parent / "targets" / "matrix.json"
    return json.loads(p.read_text())
