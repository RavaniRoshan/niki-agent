"""Rebrand the first-run onboarding copy.

Found by running the real thing. The app opens a `LaunchNameScreen` on a fresh
profile asking what to call you, and it greeted you as "Welcome to Deep Agents
Code" -- so a first-time Niki user never reached the Niki UI at all. They were
stuck on an upstream-branded modal.

These strings are patched rather than substituted from `NikiApp`. `app.py`
imports `LaunchNameScreen` *inside functions* (:6865, :12292, :12337), so there is
no module attribute to rebind -- the same dead end as `ApprovalMenu`, and only a
real run surfaced that this screen exists at all.

`niki/product.py` holds the product name so the copy and the version line cannot
disagree.
"""

import pathlib

launch = pathlib.Path("deepagents_code/tui/widgets/launch_init.py")
s = launch.read_text(encoding="utf-8")

pairs = [
    (
        'yield Static("Welcome to Deep Agents Code", classes="launch-init-title")',
        'yield Static(f"Welcome to {PRODUCT_NAME}", classes="launch-init-title")',
    ),
    (
        'Content.assemble("What should Deep Agents call you?")',
        'Content.assemble(f"What should {PRODUCT_NAME} call you?")',
    ),
    (
        '"When you create or update a goal, dcode drafts acceptance "',
        'f"When you create or update a goal, {PRODUCT_NAME} drafts acceptance "',
    ),
]
for old, new in pairs:
    if old in s:
        s = s.replace(old, new)
        print(f"patched: {old[:48]}...")

# Add the import next to the module's other imports.
anchor = "from deepagents_code.tui.widgets"
if "from deepagents_code.niki.product import PRODUCT_NAME" not in s:
    lines = s.splitlines(keepends=True)
    last = max(i for i, line in enumerate(lines) if line.startswith("from ") or line.startswith("import "))
    lines.insert(last + 1, "from deepagents_code.niki.product import PRODUCT_NAME\n")
    s = "".join(lines)

launch.write_text(s, encoding="utf-8")
print("done")