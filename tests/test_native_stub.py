"""Keep the handwritten native stub aligned with the compiled extension."""

from __future__ import annotations

import ast
from pathlib import Path

from kfnetlist import _native


def test_native_stub_covers_runtime_surface() -> None:
    stub = Path(_native.__file__).with_name("_native.pyi")
    module = ast.parse(stub.read_text(encoding="utf-8"))
    definitions = {
        node.name
        for node in module.body
        if isinstance(node, (ast.ClassDef, ast.FunctionDef))
    }
    runtime = {name for name in dir(_native) if not name.startswith("_")}
    assert runtime <= definitions


def test_native_stub_methods_exist_at_runtime() -> None:
    stub = Path(_native.__file__).with_name("_native.pyi")
    module = ast.parse(stub.read_text(encoding="utf-8"))
    for node in module.body:
        if not isinstance(node, ast.ClassDef) or not hasattr(_native, node.name):
            continue
        runtime_class = getattr(_native, node.name)
        methods = {
            item.name
            for item in node.body
            if isinstance(item, ast.FunctionDef) and not item.name.startswith("_")
        }
        assert methods <= set(dir(runtime_class)), node.name
