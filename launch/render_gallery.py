#!/usr/bin/env python3
"""Render the Product Hunt gallery from the README recording.

Every image is 1270x760 (Product Hunt's gallery ratio) and uses only the
project's own colours and screenshots. Run from the repository root:

    python3 launch/render_gallery.py

The terminal frames come from intro.gif, so re-record the GIF before re-running
this script when the UI changes.
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "launch" / "gallery"
GIF = ROOT / "intro.gif"

W, H = 1270, 760
BG = (0x10, 0x10, 0x14)
PANEL = (0x1b, 0x1c, 0x22)
BODY = (0xe6, 0xe7, 0xec)
MUTED = (0xaa, 0xb0, 0xbd)
ACCENT = (0x8e, 0xc7, 0xff)
OK = (0xa8, 0xe6, 0xa1)
BORDER = (0x33, 0x35, 0x3d)

SANS = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
SANS_B = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
MONO = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"
MONO_B = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"


def font(path, size):
    return ImageFont.truetype(path, size)


def canvas():
    img = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(img)
    d.text((56, 40), "niki", font=font(MONO_B, 26), fill=BODY)
    d.ellipse((120, 45, 130, 55), fill=ACCENT)
    d.text((W - 56 - 210, 44), "niki-site.vercel.app", font=font(MONO, 16), fill=MUTED)
    d.line((56, 86, W - 56, 86), fill=BORDER)
    return img, d


def wrap(d, text, fnt, width):
    words, lines, line = text.split(), [], ""
    for word in words:
        trial = (line + " " + word).strip()
        if d.textlength(trial, font=fnt) <= width:
            line = trial
        else:
            if line:
                lines.append(line)
            line = word
    if line:
        lines.append(line)
    return lines


def paragraph(d, xy, text, fnt, width, fill=MUTED, leading=8):
    x, y = xy
    for line in wrap(d, text, fnt, width):
        d.text((x, y), line, font=fnt, fill=fill)
        y += fnt.size + leading
    return y


def gif_frame(index):
    im = Image.open(GIF)
    im.seek(index)
    return im.convert("RGB")


def terminal_crop(index, box, size):
    """Crop the terminal window out of a recorded frame and scale it."""
    fr = gif_frame(index)
    crop = fr.crop(box)
    return crop.resize(size, Image.LANCZOS)


def framed(img, shot, xy):
    """Place a screenshot in a rounded panel with a hairline border."""
    x, y = xy
    panel = Image.new("RGBA", (shot.width + 24, shot.height + 24), (0, 0, 0, 0))
    pd = ImageDraw.Draw(panel)
    pd.rounded_rectangle((0, 0, panel.width - 1, panel.height - 1), radius=14,
                         fill=PANEL + (255,), outline=BORDER + (255,))
    panel.paste(shot, (12, 12))
    img.paste(panel.convert("RGB"), (x, y), panel)


def title_block(d, kicker, title, subtitle, width=620):
    d.text((56, 140), kicker.upper(), font=font(MONO_B, 16), fill=ACCENT)
    y = 172
    for line in wrap(d, title, font(SANS_B, 54), width):
        d.text((56, y), line, font=font(SANS_B, 54), fill=BODY)
        y += 64
    paragraph(d, (56, y + 18), subtitle, font(SANS, 22), width, leading=9)


def bullets(d, xy, items, width, size=21, gap=54):
    x, y = xy
    for head, text in items:
        d.text((x, y), head, font=font(SANS_B, size), fill=BODY)
        paragraph(d, (x, y + size + 8), text, font(SANS, 17), width, leading=5)
        y += gap + 22 * (len(wrap(d, text, font(SANS, 17), width)) - 1)
    return y


def gallery_1():
    img, d = canvas()
    title_block(
        d,
        "Niki Agent",
        "A quiet coding agent.",
        "One native binary. Eleven providers. Sessions on your disk. An approval gate "
        "in front of every action.",
        width=420,
    )
    shot = terminal_crop(27, (128, 76, 128 + 944, 76 + 648), (700, 480))
    framed(img, shot, (520, 210))
    d.text((56, H - 92), "github.com/RavaniRoshan/niki-agent",
           font=font(MONO, 17), fill=MUTED)
    img.save(OUT / "01-hero.png")


def gallery_2():
    img, d = canvas()
    title_block(
        d,
        "Distribution",
        "One binary. No runtime.",
        "The turn loop, providers, tools, policy, sessions, MCP client, and UI are one "
        "Rust executable. There is nothing to compile and nothing to resolve.",
    )
    d.text((56, 420), "1", font=font(SANS_B, 120), fill=ACCENT)
    d.text((120, 470), "executable", font=font(MONO, 22), fill=BODY)
    d.text((56, 560), "64-bit Linux, WSL, macOS, Windows", font=font(SANS, 20), fill=MUTED)

    box = (640, 180, 1214, 560)
    d.rounded_rectangle(box, radius=14, fill=PANEL, outline=BORDER)
    lines = [
        ("$ curl -fsSL .../install.sh | bash", BODY),
        ("  downloading the linux-x86_64 release", MUTED),
        ("  verifying SHA256SUMS", MUTED),
        ("  installed to ~/.local/niki", OK),
        ("$ niki --version", BODY),
        ("niki 2.1.3", BODY),
    ]
    y = 220
    for text, col in lines:
        d.text((676, y), text, font=font(MONO, 17), fill=col)
        y += 40
    d.text((56, H - 92), "MIT licensed · no account · no telemetry",
           font=font(MONO, 17), fill=MUTED)
    img.save(OUT / "02-one-binary.png")


def gallery_3():
    img, d = canvas()
    title_block(
        d,
        "Approval gate",
        "It asks before it acts.",
        "Manual asks for every gated command. Auto runs the allow-list and asks for the "
        "rest. YOLO runs what the policy allows. A workspace boundary keeps file tools "
        "inside the project.",
        width=560,
    )
    modes = [("Manual", "asks for every gated call"),
             ("Auto", "allow-list, then ask"),
             ("YOLO", "everything the policy allows")]
    y = 440
    for name, text in modes:
        d.rounded_rectangle((56, y, 56 + 460, y + 62), radius=10, fill=PANEL,
                            outline=BORDER)
        d.text((76, y + 12), name, font=font(SANS_B, 20), fill=BODY)
        d.text((76, y + 36), text, font=font(SANS, 15), fill=MUTED)
        y += 76

    shot = terminal_crop(27, (300, 280, 300 + 660, 280 + 420), (640, 400))
    framed(img, shot, (560, 250))
    d.text((56, H - 68), "y allow once · s allow session · d refuse",
           font=font(MONO, 17), fill=MUTED)
    img.save(OUT / "03-approval-gate.png")


def gallery_4():
    img, d = canvas()
    title_block(
        d,
        "Providers and files",
        "Your model. Your files.",
        "Anthropic, OpenAI, Gemini, Groq, OpenRouter, Together, DeepSeek, NVIDIA, "
        "Mistral, xAI, and Ollama are built in. Any OpenAI-compatible endpoint is one "
        "TOML block.",
        width=600,
    )
    chips = ["anthropic", "openai", "gemini", "groq", "openrouter", "together",
             "deepseek", "nvidia", "mistral", "xai", "ollama"]
    x, y = 56, 450
    for chip in chips:
        width = d.textlength(chip, font=font(MONO, 16)) + 28
        if x + width > 700:
            x, y = 56, y + 46
        d.rounded_rectangle((x, y, x + width, y + 34), radius=17, outline=BORDER)
        d.text((x + 14, y + 8), chip, font=font(MONO, 16), fill=BODY)
        x += width + 10

    box = (760, 180, 1214, 560)
    d.rounded_rectangle(box, radius=14, fill=PANEL, outline=BORDER)
    code = [
        ("[models]", ACCENT),
        ('default = "groq:llama-3.3-70b"', BODY),
        ("", BODY),
        ("[providers.moonshot]", ACCENT),
        ('base_url = "https://api.moonshot.ai/v1"', BODY),
        ('api_key_env = "MOONSHOT_API_KEY"', BODY),
        ("", BODY),
        ("~/.niki/", MUTED),
        ("  config.toml", MUTED),
        ("  sessions.db", MUTED),
        ("  skills/", MUTED),
    ]
    y = 220
    for text, col in code:
        d.text((794, y), text, font=font(MONO, 16), fill=col)
        y += 30
    img.save(OUT / "04-providers.png")


def gallery_5():
    img, d = canvas()
    title_block(
        d,
        "Long sessions",
        "Built for the long session.",
        "The composer queues while a turn runs. The transcript scrolls back to the first "
        "prompt. Sessions are named, forked, archived, and exported as Markdown.",
        width=560,
    )
    bullets(d, (56, 440), [
        ("Ctrl+T", "the whole conversation, scrollable, without leaving the composer"),
        ("/diff", "staged, unstaged, and untracked changes in the transcript"),
        ("/compact", "fold older turns through the model instead of dropping them"),
        ("/theme", "four palettes measured for contrast in the test suite"),
    ], 520, size=20, gap=58)

    shot = terminal_crop(42, (128, 76, 128 + 944, 76 + 648), (640, 440))
    framed(img, shot, (620, 200))
    d.text((56, H - 60), "sessions fork · sessions archive · sessions list --json",
           font=font(MONO, 16), fill=MUTED)
    img.save(OUT / "05-long-sessions.png")


def thumbnail():
    img = Image.new("RGB", (240, 240), BG)
    d = ImageDraw.Draw(img)
    d.ellipse((44, 44, 84, 84), outline=ACCENT, width=5)
    d.pieslice((44, 44, 84, 84), 90, 270, fill=ACCENT)
    d.text((44, 108), "niki", font=font(MONO_B, 44), fill=BODY)
    d.text((44, 170), "terminal coding agent", font=font(MONO, 12), fill=MUTED)
    img.save(OUT / "thumbnail-240.png")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    gallery_1()
    gallery_2()
    gallery_3()
    gallery_4()
    gallery_5()
    thumbnail()
    for path in sorted(OUT.iterdir()):
        print(f"{path.name}: {path.stat().st_size / 1000:.0f} kB")


if __name__ == "__main__":
    main()
