#!/usr/bin/env python3
"""Validate embedded translations without third-party Python dependencies."""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
fields = re.compile(r"\{([0-9]+)\}")
messages = json.loads((ROOT / "locales/messages.json").read_text())
widgets = json.loads((ROOT / "locales/widgets.json").read_text())
for key, values in messages.items():
    assert len(values) == 8, key
    for value in values:
        assert value, key
        assert sorted(fields.findall(key)) == sorted(fields.findall(value)), (key, value)
for key, values in widgets.items():
    assert len(values) == 8 and all(values), key

literal = r'"(?:[^"\\]|\\.)*"'
missing = []
for source in (ROOT / "src").rglob("*.rs"):
    if source.name == "i18n.rs":
        continue
    text = source.read_text().split("#[cfg(test)]")[0]
    for match in re.finditer(r"\b(?:tr|tf)\(\s*(" + literal + r")", text):
        key = json.loads(match[1])
        if re.search(r"[\u4e00-\u9fff]", key) and key not in messages:
            missing.append((str(source.relative_to(ROOT)), key))
assert not missing, missing
print(f"Validated {len(messages)} application messages and {len(widgets)} widget labels in 8 languages.")
