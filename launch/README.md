# Launch assets

This directory holds the product-launch material. Nothing here is referenced by
the `niki` binary or by the installers.

| Path | What it is |
| --- | --- |
| `product-hunt.md` | Listing copy, maker comment, FAQ, and the day-of checklist |
| `gallery/01-hero.png` … `05-long-sessions.png` | 1270×760 gallery images |
| `gallery/thumbnail-240.png` | 240×240 listing thumbnail |
| `render_gallery.py` | Rebuilds the gallery from `../intro.gif` |

## Rebuilding the gallery

The screenshots come from the README recording, so the two stay in sync:

```bash
python3 launch/render_gallery.py
```

The script reads `intro.gif` and writes `launch/gallery/`. It needs Pillow and
the DejaVu fonts; nothing else. Frame indices for the approval card and the
final answer are chosen in the script and may need updating after a re-record.

## Rules for this copy

- Match shipped behaviour. Features under "Known gaps" never appear as shipped.
- No comparison that names another product. Say what Niki does, not what it is
  like.
- The install commands and version numbers must match `install/install.sh` and
  the newest tag.
