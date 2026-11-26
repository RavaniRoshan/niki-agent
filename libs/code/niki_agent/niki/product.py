"""The product name, in one place.

The onboarding, version line, and headers each used to name the product
independently. They now share this constant so the copy cannot disagree
with the version line. Kept separate from `branding.py` because onboarding runs
before the rest of Niki is necessarily loaded, and an import cycle through
`branding` would be the wrong way to solve that.
"""

from __future__ import annotations

from typing import Final

PRODUCT_NAME: Final = "Niki Agent"
"""What the product calls itself in every user-visible surface."""
