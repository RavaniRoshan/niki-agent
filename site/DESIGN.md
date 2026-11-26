# Niki Agent Design Constitution (`DESIGN.md`)

Single source of truth for **Niki Agent**'s visual architecture, OKLCH color system, Geist typography scale, symmetrical hairline-grid structure, normalized component taxonomy, copy budget, and GSAP/Stageshow motion rules. Every section on the page obeys this document so the entire experience reads as one unified instrument rather than patchwork.

---

## 1. Symmetrical Architectural Hairline Grid

Every section—from the fixed Navbar through Hero, Feature Levels 01–04, and the Ticket Stub Footer—shares one continuous vertical and horizontal hairline blueprint:

| Structural Rule | Token / Implementation | Visual Role |
| :--- | :--- | :--- |
| Symmetrical Outer Rails | `--grid-rail: clamp(16px, 2.5vw, 36px)` | Identical left & right architectural margin tracks aligning Navbar, Hero, Bento, Parallax, Orbital Dial, Benchmarks, Proof Stats, and TicketStubFooter. |
| Hairline Rules | `--grid-line` / `--grid-line-strong` | 1px crisp structural seams (`12%–14%` foreground mix) separating bands and abutting grid cells. |
| Intersection Nodes | `7px` rotated hairline diamonds (`◇`) | Quiet architectural corner registration marks at outer band intersections. |
| Section Eyebrow Bar | `<SectionEyebrow index="01" label="..." />` | Unified full-width ruled header (`◇ 01 · SECTION NAME ········ META`) echoing the Ticket Stub Footer eyebrow across every level. |
| Band Wrapper | `<GridBand>` (`src/shared/ui/DesignPrimitives.tsx`) | Mandatory structural frame for all landing sections. Never use asymmetric single-side rails or disconnected floating `max-w` wrappers. |

---

## 2. Color Schema (Active OKLCH Dark-Default + Light Mode)

All semantic colors live in `src/index.css` (`:root` for Light Mode, `.dark` for Default Dark Mode, `.surface-light-band` for the editorial high-contrast proof band).

| Role | Token | Dark Default (`.dark`) | Light Mode (`:root`) | Usage |
| :--- | :--- | :--- | :--- | :--- |
| Primary Canvas | `--background` | `oklch(0 0 0)` | `oklch(0.99 0.002 281.25)` | Page ground, grid bands, footer ground |
| Primary Ink | `--foreground` | `oklch(0.974 0.004 281.223)` | `oklch(0.12 0.031 273.513)` | Display headings, active labels, hairline ink base |
| Elevated Cell | `--card` | `oklch(0.092 0 180)` | `oklch(0.981 0.005 281.213)` | Grid cells, dialogs, popovers, attached navbar |
| Muted Fill | `--muted` | `oklch(0.181 0.009 237.202)` | `oklch(0.946 0.006 236.526)` | Active tabs, hover states, code headers |
| Muted Copy | `--muted-foreground` | `oklch(0.789 0.011 236.635)` | `oklch(0.39 0.013 236.931)` | Supporting copy, secondary labels, timestamps |
| Structural Border | `--border` / `--grid-line` | `oklch(0.254 0.02 279.938)` / `14% ink` | `oklch(0.941 0.006 281.176)` / `12% ink` | Hairline grid seams, button outlines, modal frames |
| Primary Accent | `--primary` / `--chart-1` | `oklch(0.473 0.283 265.93)` | `oklch(0.473 0.283 265.93)` | Primary CTAs, Ticket Stub fill, focus rings, active indicators |
| Secondary Accent | `--secondary` / `--chart-2` | `oklch(0.676 0.168 245.961)` | `oklch(0.676 0.168 245.961)` | Dark-mode high-contrast mono highlights & telemetry |
| Tertiary Accent | `--accent` / `--chart-3` | `oklch(0.513 0.279 289.739)` | `oklch(0.513 0.279 289.739)` | Atmospheric shader highlights |

### Adaptive Surface Rhythm
1. **Default Dark Mode (`.dark`)**: Pure obsidian canvas across Hero, Levels 01–03, and Ticket Stub Footer, contrasted by the mid-page `.surface-light-band` (Level 04 Benchmarks & Proof Stats) where the attached navbar transitions automatically to light glass.
2. **Global Light Mode Toggle**: Switching themes flips every semantic token, hairline grid seam, Three.js shader uniforms, and Ticket Stub Footer surface cleanly with WCAG AA contrast.

---

## 3. Typography System (Geist Sans + Geist Mono)

Strictly locked to **Geist** (`--font-sans`) and **Geist Mono** (`--font-mono`) with one editorial serif exception reserved solely for the halftone footer canvas wordmark.

| Element | Font | Weight | Size | Tracking / Rules |
| :--- | :--- | :--- | :--- | :--- |
| Hero Heading (`h1`) | Geist Sans | `600` | `64px–84px` (`52px` mobile) | `-0.04em`, `leading-[1.02]`, `text-wrap: balance` |
| Section Heading (`h2`) | Geist Sans | `600` | `36px–52px` | `-0.03em`, `leading-[1.08]`, `text-wrap: balance` |
| Feature / Card Heading (`h3`) | Geist Sans | `550` | `20px` | `-0.02em`, `leading-[1.3]` (`.font-feature-heading`) |
| Body Copy | Geist Sans | `400` | `16px–18px` | `leading-[1.55]` |
| Navigation & Buttons | Geist Sans | `500` | `14px` | `whitespace-nowrap shrink-0` |
| Eyebrow Labels | Geist Mono | `500` | `11px–12px` | `tracking-[0.06em]`, `tabular-nums` |
| Terminal / Code | Geist Mono | `400` | `13px–14px` | `leading-relaxed` |
| Keyboard Shortcuts (`<Keycap>`) | Geist Mono | `500` | `11px–12px` | Square `2px` architectural keycap |

---

## 4. Normalized Button System (`BTN` in `DesignPrimitives.tsx`)

No ad-hoc pill buttons (`rounded-full`) or mismatched corner radii. Every interactive action maps to one of five strict variants using crisp `2px` architectural precision (`rounded-[2px]`):

| Variant | Token | Visual Grammar | Allowed Usage |
| :--- | :--- | :--- | :--- |
| **Primary** | `BTN.primary` | Solid Royal Cobalt (`bg-primary text-primary-foreground`), `h-10 px-5`, `active:scale-[0.98]` | Navbar `Install Niki [I]`, Hero `Install Niki`, Benchmarks `Inspect full benchmarks`, Modal primary CTA |
| **Secondary** | `BTN.secondary` | Crisp 1px hairline outline (`border-grid-strong bg-card/70`), `h-10 px-4` | Navbar `GitHub`, Modal secondary action, Mega Menu full-width directory links |
| **Subtle Mono** | `BTN.subtle` | `h-9 px-3.5`, `font-tech text-[12px]`, 1px hairline border (`border-grid`) | Orbital stage deep-dive links (`↗`), copy snippet buttons, drawer toggles |
| **Ghost Nav** | `BTN.ghost` | Unboxed `14px` Geist Sans `500`, quiet hover surface (`hover:bg-muted/70`) | Navbar triggers (`Providers`, `Architecture`, `Resources`, `Docs`, `Benchmarks`) |
| **Square Icon** | `BTN.icon` / `BTN.iconLg` | Square `36px` or `40px` hairline cell (`rounded-[2px] border-grid`) | Theme toggle, Bento inspect `↗`, Carousel Prev/Next arrows, Modal close `×` |

- **Chamfered Closing CTA**: Reserved exclusively for the Ticket Stub Footer hero CTA (`.tsf-cta`) as the closing architectural signature.
- **Segmented Controls**: 1px hairline-framed bar (`border border-grid bg-background p-0.5 rounded-[2px]`) with crisp active fill (`bg-primary text-primary-foreground`) for state toggles (`Active Stream / Idle Stillness`, mobile stage tabs, Hero presets).

---

## 5. Normalized Card & Surface Taxonomy (`CARD` in `DesignPrimitives.tsx`)

Eliminate floating `rounded-2xl` bubble cards and nested cards-within-cards. Every content block uses one of three architectural archetypes:

1. **Hairline Grid Cell (`CARD.cell` / `CARD.cellInteractive`)**
   - Sharp `rounded-none` cells tiled edge-to-edge on shared 1px `--grid-line` seams (zero floating gaps or drop shadows).
   - Used across: Level 01 Bento (7+5 / 5+7 matrix), Level 02 Parallax chapter rail, Level 04 Benchmark 3-column deck, Pillar 3-column strip, Proof Stats 3-column strip, and Navbar 3-column Mega Menus.
2. **Stageshow Viewport Frame (`CARD.stageFrame`)**
   - Flush recessed obsidian terminal stage (`bg-[#06070a]`) bound directly by hairline seams (`rounded-none`).
   - Primary cropped terminal windows bleed cleanly off the right/bottom edges (`rounded-none` / crisp 1px `#282e46` chrome) with straddling depth satellite readouts (`rounded-[2px]`).
3. **Architectural Dialog (`CARD.modal`)**
   - Sharp `rounded-[2px]` bordered overlay (`border-grid-strong bg-card`) featuring the unified `<SectionEyebrow>` top bar, Geist `600` heading, hairline-framed code blocks, and normalized `BTN.primary` / `BTN.secondary` actions.

---

## 6. Copy Density Budget & Brand Authenticity

Complex developer tools stay scannable when copy obeys tight slot limits:

| Slot | Budget | Rule |
| :--- | :--- | :--- |
| Hero `h1` | 3–5 words | Conceptual anchor (`Meet your calm coding agent`) |
| Section `h2` | 5–8 words | Concrete engineering claim, max 2 lines |
| Feature Card `h3` + body | 5–8 word title · 1–2 short sentences | Visual Stageshow UI carries 60% of the explanation |
| Orbital Stage | 1-word label · 3 bullets (`Bold Verb` + 6–11 words) | Deep TOML specs open on demand in dialogs |
| Proof Stats | 3 tabular numbers + 3–5 word label | Exact, checkable values (`4`, `1`, `3`) |

- **Authentic Third-Party Marks**: Always render real vector brand marks for supported providers and runtimes (`Anthropic`, `OpenAI`, `Google`, `Ollama`, `OpenRouter`, `NVIDIA`, `Groq`, `Together`, `DeepSeek`, `Mistral`, `xAI`, `Rust`, `Cargo`, `ratatui`, `SQLite`, `MCP`, `GitHub`). Never invent fake company names.

---

## 7. Motion, Performance & Accessibility Contract

- **Engine**: GSAP Core (`gsap` + `ScrollTrigger`) for scroll-scrubbed orbital dial sequencing, 3-depth parallax chapters, and timestamped reveals; CSS compositor transitions (`150ms–250ms`) for routine UI feedback.
- **Compositor-Only Rule**: Animate strictly GPU-composited properties (`transform`, `opacity`, `clip-path`, `color`, `background-color`, `border-color`). Never animate `width`, `height`, `padding`, or `margin` on scroll or hover.
- **Zero Per-Pixel React Re-renders**: Scroll-scrubbed progress bars, parallax depth offsets, orbital node positions, and navbar scroll thresholds update DOM refs or 1-bit threshold state—never 60Hz React `setState`.
- **Accessibility & Stillness**: Respect `prefers-reduced-motion: reduce` across all GSAP timelines, CSS animations, and canvases; pause offscreen canvas/interval loops via `IntersectionObserver`; lock background scroll on open dialogs; enforce `rel="noopener noreferrer"` on all external links and protocol sanitization on dynamic `href`s.
