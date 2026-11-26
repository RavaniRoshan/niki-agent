export interface EcosystemLogoItem {
  id: string;
  name: string;
  actionType?: 'Extra' | 'Config' | 'Runtime';
  extraName?: string;
  envVar?: string;
  summary?: string;
  configSnippet?: string;
}

export interface LifecycleStage {
  id: 'boot' | 'still' | 'theme' | 'bind' | 'scale' | 'route';
  label: string;
  headline: string;
  bullets: { lead: string; text: string }[];
  links: { label: string; detailId: string }[];
}

export interface PillarCardData {
  id: 'niki-ui' | 'niki-themes' | 'niki-providers';
  packageName: string;
  title: string;
  subtitle: string;
  ctaLabel: string;
  installCmd: string;
  codeSnippet: string;
  highlights: string[];
}

export interface BenchmarkStory {
  id: 'redraws' | 'render-cost' | 'memory' | 'contrast' | 'keymap';
  badge: string;
  headline: string;
  metric: string;
  before: string;
  after: string;
  beforePct: number;
  afterPct: number;
  method: string;
  verification: string;
  command: string;
}

export interface FeatureDetail {
  id: string;
  name: string;
  category: 'Presentation Layer' | 'Architecture' | 'Providers & Docs';
  tagline: string;
  description: string;
  capabilities: string[];
  codePreview?: string;
}

export interface NikiThemePalette {
  id: 'niki' | 'niki-light' | 'niki-contrast' | 'niki-dim' | 'no-color';
  name: string;
  description: string;
  tag: string;
  bg: string;
  surface: string;
  border: string;
  text: string;
  muted: string;
  accent: string;
}

export interface ParallaxChapter {
  id: 'stillness' | 'mount-window' | 'accessibility' | 'independence';
  indexLabel: string;
  title: string;
  summary: string;
  metricValue: string;
  metricLabel: string;
  detailId: string;
}

export interface HeroQuickPreset {
  id: string;
  label: string;
  command: string;
  placeholder: string;
}

export const INSTALL_COMMAND =
  'curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash';

export const HERO_QUICK_PRESETS: HeroQuickPreset[] = [
  {
    id: 'install',
    label: 'Install',
    command: INSTALL_COMMAND,
    placeholder: INSTALL_COMMAND,
  },
  {
    id: 'anthropic',
    label: 'Anthropic',
    command: 'export ANTHROPIC_API_KEY=sk-ant-... && niki',
    placeholder: 'export ANTHROPIC_API_KEY=sk-ant-... && niki',
  },
  {
    id: 'openai',
    label: 'OpenAI',
    command: 'export OPENAI_API_KEY=sk-... && niki',
    placeholder: 'export OPENAI_API_KEY=sk-... && niki',
  },
  {
    id: 'google',
    label: 'Google',
    command: 'export GOOGLE_API_KEY=... && niki',
    placeholder: 'export GOOGLE_API_KEY=... && niki',
  },
  {
    id: 'ollama',
    label: 'Ollama',
    command: '/install ollama',
    placeholder: '/install ollama',
  },
];

export const PARALLAX_CHAPTERS: ParallaxChapter[] = [
  {
    id: 'stillness',
    indexLabel: '01',
    title: 'Stillness at rest',
    summary:
      'Timers pause when the agent is waiting on you: the spinner and status widgets disarm on input idle, and the perf probe asserts an idle session paints nothing.',
    metricValue: 'stillness',
    metricLabel: 'asserted in tests',
    detailId: 'baseline',
  },
  {
    id: 'mount-window',
    indexLabel: '02',
    title: 'Bounded mount window',
    summary:
      'Historical turns leave the mounted window while the newest stream stays live, so hour two renders like minute one.',
    metricValue: 'bounded',
    metricLabel: 'offscreen turns unmounted',
    detailId: 'mount-window',
  },
  {
    id: 'accessibility',
    indexLabel: '03',
    title: 'Shell-native accessibility',
    summary:
      'Honours NIKI_REDUCED_MOTION, NO_COLOR, NIKI_UI_CHARSET_MODE=ascii, and NIKI_SCREEN_READER without degrading composer clarity.',
    metricValue: '4 themes',
    metricLabel: 'contrast checked in tests',
    detailId: 'adaptive-motion',
  },
  {
    id: 'independence',
    indexLabel: '04',
    title: 'One package, one command',
    summary:
      'The agent loop, tool registry, approval policy, and editor protocol are implemented in this repository. State lives in ~/.niki, and niki is the only console script.',
    metricValue: '1 command',
    metricLabel: 'one package · one state dir',
    detailId: 'independence',
  },
];

export const NIKI_THEMES: NikiThemePalette[] = [
  {
    id: 'niki',
    name: 'niki',
    description: 'The default. Cool, flat greys, one accent.',
    tag: 'DEFAULT',
    bg: '#0c1017',
    surface: '#131a24',
    border: '#253142',
    text: '#e6edf6',
    muted: '#8b9bb0',
    accent: '#7fc8ff',
  },
  {
    id: 'niki-light',
    name: 'niki-light',
    description: 'Light terminals with crisp ink contrast.',
    tag: 'LIGHT',
    bg: '#f4f7fb',
    surface: '#ffffff',
    border: '#cdd9e8',
    text: '#101824',
    muted: '#4d5d73',
    accent: '#1575e6',
  },
  {
    id: 'niki-contrast',
    name: 'niki-contrast',
    description: 'Bright rooms and low-quality panels.',
    tag: 'BRIGHT ROOMS',
    bg: '#05080d',
    surface: '#0d131c',
    border: '#4c6585',
    text: '#ffffff',
    muted: '#b9cce0',
    accent: '#9bd8ff',
  },
  {
    id: 'niki-dim',
    name: 'niki-dim',
    description: 'Dark rooms, softer than the default.',
    tag: 'DIM ROOMS',
    bg: '#13171f',
    surface: '#1a202b',
    border: '#2d3748',
    text: '#cbd5e1',
    muted: '#8494a7',
    accent: '#6db2e8',
  },
];

export const ECOSYSTEM_LOGOS_ROW_1: EcosystemLogoItem[] = [
  {
    id: 'anthropic',
    name: 'Anthropic',
    actionType: 'Extra',
    extraName: 'anthropic',
    envVar: 'ANTHROPIC_API_KEY',
    summary: 'Installed with the app. Set a key and pick a Claude model in /model; nothing else to install.',
    configSnippet: `export ANTHROPIC_API_KEY=sk-ant-...\nniki`,
  },
  {
    id: 'openai',
    name: 'OpenAI',
    actionType: 'Extra',
    extraName: 'openai',
    envVar: 'OPENAI_API_KEY',
    summary: 'Installed with the app, including Azure OpenAI and ChatGPT subscription login.',
    configSnippet: `export OPENAI_API_KEY=sk-...\nniki`,
  },
  {
    id: 'google',
    name: 'Google',
    actionType: 'Extra',
    extraName: 'google-genai',
    envVar: 'GOOGLE_API_KEY',
    summary: 'Gemini models are installed with the app; one key environment variable is all they need.',
    configSnippet: `export GOOGLE_API_KEY=...\nniki`,
  },
  {
    id: 'ollama',
    name: 'Ollama',
    actionType: 'Extra',
    extraName: 'ollama',
    envVar: 'OLLAMA_HOST',
    summary: 'Local models, offline. Install the integration once, then /model lists what you have pulled.',
    configSnippet: `/install ollama\n/model`,
  },
  {
    id: 'openrouter',
    name: 'OpenRouter',
    actionType: 'Extra',
    extraName: 'openrouter',
    envVar: 'OPENROUTER_API_KEY',
    summary: 'One integration, hundreds of routed models.',
    configSnippet: `/install openrouter\nexport OPENROUTER_API_KEY=sk-or-...`,
  },
  {
    id: 'nvidia',
    name: 'NVIDIA',
    actionType: 'Extra',
    extraName: 'nvidia',
    envVar: 'NVIDIA_API_KEY',
    summary: 'NVIDIA NIM endpoints as an optional integration.',
    configSnippet: `/install nvidia\nexport NVIDIA_API_KEY=nvapi-...`,
  },
  {
    id: 'groq',
    name: 'Groq',
    actionType: 'Extra',
    extraName: 'groq',
    envVar: 'GROQ_API_KEY',
    summary: 'Low-latency LPU inference as an optional integration.',
    configSnippet: `/install groq\nexport GROQ_API_KEY=gsk_...`,
  },
  {
    id: 'together',
    name: 'Together',
    actionType: 'Extra',
    extraName: 'together',
    envVar: 'TOGETHER_API_KEY',
    summary: 'Open-weights coding models as an optional integration.',
    configSnippet: `/install together\nexport TOGETHER_API_KEY=...`,
  },
  {
    id: 'deepseek',
    name: 'DeepSeek',
    actionType: 'Extra',
    extraName: 'deepseek',
    envVar: 'DEEPSEEK_API_KEY',
    summary: 'DeepSeek reasoning and coding models as an optional integration.',
    configSnippet: `/install deepseek\nexport DEEPSEEK_API_KEY=...`,
  },
  {
    id: 'kimi',
    name: 'Kimi',
    actionType: 'Config',
    envVar: 'MOONSHOT_API_KEY',
    summary: 'An OpenAI-compatible gateway is a config block, not an install: add it to ~/.niki/config.toml and it appears in /model.',
    configSnippet: `# ~/.niki/config.toml\n[models.providers.kimi]\ndisplay_name = "Kimi"\nclass_path = "langchain_openai:ChatOpenAI"\nbase_url = "https://api.moonshot.ai/v1"\napi_key_env = "MOONSHOT_API_KEY"\nmodels = ["kimi-k3", "kimi-k2.7-code", "kimi-k2.6"]`,
  },
];

export const ECOSYSTEM_LOGOS_ROW_2: EcosystemLogoItem[] = [
  { id: 'python', name: 'Python 3.12+' },
  { id: 'uv', name: 'uv' },
  { id: 'textual', name: 'Textual 8' },
  {
    id: 'kilo',
    name: 'Kilo',
    actionType: 'Config',
    envVar: 'KILO_API_KEY',
    summary: 'A gateway block in ~/.niki/config.toml using langchain_openrouter:ChatOpenRouter.',
    configSnippet: `# ~/.niki/config.toml\n[models.providers.kilo]\ndisplay_name = "Kilo AI Gateway"\nclass_path = "langchain_openrouter:ChatOpenRouter"\nbase_url = "https://api.kilo.ai/api/gateway"\napi_key_env = "KILO_API_KEY"\nmodels = ["anthropic/claude-opus-4.7", "openai/gpt-5.4", "moonshotai/kimi-k2.5"]`,
  },
  {
    id: 'opencode',
    name: 'OpenCode Zen',
    actionType: 'Config',
    envVar: 'OPENCODE_API_KEY',
    summary: 'An OpenAI-compatible gateway block, configured entirely in ~/.niki/config.toml.',
    configSnippet: `# ~/.niki/config.toml\n[models.providers.opencode_zen]\ndisplay_name = "OpenCode Zen"\nclass_path = "langchain_openai:ChatOpenAI"\nbase_url = "https://opencode.ai/zen/v1"\napi_key_env = "OPENCODE_API_KEY"\nmodels = ["kimi-k3", "deepseek-v4-pro", "glm-5.3"]`,
  },
  { id: 'pytest', name: 'pytest' },
  { id: 'github', name: 'GitHub' },
];

export const HERO_FEATURED_LOGOS: EcosystemLogoItem[] = [
  ECOSYSTEM_LOGOS_ROW_1[0], // Anthropic
  ECOSYSTEM_LOGOS_ROW_1[1], // OpenAI
  ECOSYSTEM_LOGOS_ROW_1[2], // Google
  ECOSYSTEM_LOGOS_ROW_1[3], // Ollama
  ECOSYSTEM_LOGOS_ROW_1[9], // Kimi
];

export const LIFECYCLE_STAGES: LifecycleStage[] = [
  {
    id: 'boot',
    label: 'Boot',
    headline: 'Launch straight into a working composer. No API key required upfront.',
    bullets: [
      { lead: 'Start', text: 'with one installer line, then a single command: niki' },
      { lead: 'Land', text: 'in a composer on first launch, before any provider is configured' },
      { lead: 'Add', text: 'a provider key only when you run your first task' },
    ],
    links: [
      { label: 'Install', detailId: 'install' },
      { label: 'One Package', detailId: 'independence' },
      { label: 'Agent Loop', detailId: 'agent-loop' },
    ],
  },
  {
    id: 'still',
    label: 'Still',
    headline: 'No idle redraws. A calm terminal surface that gets out of the way.',
    bullets: [
      { lead: 'Disarm', text: 'the cursor blink and status timers while waiting on input' },
      { lead: 'Replace', text: 'boxed composer chrome with a single hairline rule' },
      { lead: 'Honour', text: 'NIKI_REDUCED_MOTION, NO_COLOR, and ASCII glyph fallbacks' },
    ],
    links: [
      { label: 'Stillness', detailId: 'baseline' },
      { label: 'Adaptive Motion', detailId: 'adaptive-motion' },
    ],
  },
  {
    id: 'theme',
    label: 'Theme',
    headline: 'Four palettes with contrast checked by the test suite.',
    bullets: [
      { lead: 'Switch', text: 'live mid-session between four palettes using /theme' },
      { lead: 'Measure', text: 'text at 4.5:1 and UI glyphs at 3:1 in truecolor, 256, and 16 colours' },
      { lead: 'Read', text: 'clearly in bright rooms with niki-contrast' },
    ],
    links: [
      { label: 'Theme Registry', detailId: 'themes-system' },
      { label: 'Contrast Tests', detailId: 'ci-tests' },
    ],
  },
  {
    id: 'bind',
    label: 'Bind',
    headline: 'Keyboard-native control generated directly from the live binding registry.',
    bullets: [
      { lead: 'Approve', text: 'agent tool calls in one keystroke with 1, 2, 3, or a for auto' },
      { lead: 'Cycle', text: 'Manual → Auto → YOLO approval modes with Shift+Tab' },
      { lead: 'Open', text: 'the searchable command palette with Ctrl+Shift+P' },
    ],
    links: [
      { label: 'Live Keymap', detailId: 'keymap' },
      { label: 'Approval Modes', detailId: 'agent-loop' },
    ],
  },
  {
    id: 'scale',
    label: 'Scale',
    headline: 'A bounded mount window keeps long sessions responsive.',
    bullets: [
      { lead: 'Keep', text: 'historical turns outside the mounted window, pruned by the store' },
      { lead: 'Preserve', text: 'full scrollback and pageup/pagedown/home/end navigation' },
      { lead: 'Scroll', text: 'smoothly across a five-thousand-message transcript' },
    ],
    links: [
      { label: 'Mount Window', detailId: 'mount-window' },
      { label: 'Stillness', detailId: 'baseline' },
    ],
  },
  {
    id: 'route',
    label: 'Route',
    headline: 'Three providers out of the box, eighteen more one /install away.',
    bullets: [
      { lead: 'Connect', text: 'Anthropic, OpenAI, and Google Gemini with nothing extra to install' },
      { lead: 'Add', text: 'another integration from inside a session with /install <name>' },
      { lead: 'Define', text: 'any OpenAI-compatible endpoint as a block in ~/.niki/config.toml' },
    ],
    links: [
      { label: 'Providers Guide', detailId: 'providers' },
      { label: 'config.toml', detailId: 'providers' },
    ],
  },
];

export const PILLAR_CARDS: PillarCardData[] = [
  {
    id: 'niki-ui',
    packageName: 'niki · composer',
    title: 'Single-rule composer with adaptive phase motion',
    subtitle: 'For distraction-free, quiet terminal coding',
    ctaLabel: 'Explore the composer',
    installCmd: INSTALL_COMMAND,
    highlights: [
      'Ping-ponging ◐ ◓ ◑ ◒ spinner that only runs while the agent is working',
      'Timers disarm at rest, so an idle session paints nothing',
      'Single hairline composer rule with NO_COLOR and ASCII fallbacks',
    ],
    codeSnippet: `# Interactive terminal session (no API key needed to look around)
niki

# Accessibility & stillness environment toggles
NIKI_REDUCED_MOTION=1 niki
NO_COLOR=1 niki
NIKI_UI_CHARSET_MODE=ascii niki`,
  },
  {
    id: 'niki-themes',
    packageName: 'niki · themes',
    title: 'Four palettes switchable at runtime',
    subtitle: 'Contrast checked in truecolor, 256-colour, and 16-colour tests',
    ctaLabel: 'Explore themes',
    installCmd: '/theme niki-contrast',
    highlights: [
      'Registered directly through the Textual theme registry from one module',
      'niki, niki-light, niki-contrast, and niki-dim',
      'Tests assert 4.5:1 text and 3:1 UI glyph contrast against every surface',
    ],
    codeSnippet: `# Switch runtime palettes live inside Niki Agent
/theme niki            # cool, flat greys, one accent
/theme niki-light      # crisp light terminal palette
/theme niki-contrast   # for bright rooms and low-quality panels
/theme niki-dim        # softer luminance for dark rooms`,
  },
  {
    id: 'niki-providers',
    packageName: 'niki · providers',
    title: 'Any model, one config block away',
    subtitle: 'Three providers built in, eighteen extras, and OpenAI-compatible gateways',
    ctaLabel: 'Explore providers',
    installCmd: INSTALL_COMMAND,
    highlights: [
      'Built in: Anthropic, OpenAI (including Azure), and Google Gemini',
      'Eighteen more providers install from inside a session with /install',
      'Any OpenAI-compatible endpoint is a block in ~/.niki/config.toml',
    ],
    codeSnippet: `# ~/.niki/config.toml — zero new runtime dependencies
[models.providers.kimi]
display_name = "Kimi"
class_path = "langchain_openai:ChatOpenAI"
base_url = "https://api.moonshot.ai/v1"
api_key_env = "MOONSHOT_API_KEY"
models = ["kimi-k3", "kimi-k2.7-code", "kimi-k2.6"]`,
  },
];

export const BENCHMARK_STORIES: BenchmarkStory[] = [
  {
    id: 'redraws',
    badge: 'Idle Stillness',
    headline: 'An idle session paints nothing while it waits for you',
    metric: 'no idle redraws',
    before: 'Cursor blink timer',
    after: 'Timers disarmed at rest',
    beforePct: 100,
    afterPct: 4,
    method:
      'The perf probe counts the compositor\u2019s own repaints over a five-second resting window and asserts an idle Niki session paints no frames.',
    verification:
      'The spinner and status widgets stop on input idle, so a terminal left alone stops changing.',
    command: 'cd libs/code && uv run pytest tests/unit_tests/niki/test_perf_probes.py -q',
  },
  {
    id: 'render-cost',
    badge: 'Render Cost',
    headline: 'A bounded mount window keeps frame cost flat in a long transcript',
    metric: 'bounded window',
    before: 'Every turn mounted',
    after: 'Offscreen turns pruned',
    beforePct: 100,
    afterPct: 58,
    method:
      'The message store keeps a fixed visible window and prunes turns below it, so the newest stream is the only thing that mounts.',
    verification:
      'Scroll paint time is measured on a five-thousand-message transcript, and the mounted widget count stays bounded.',
    command: 'cd libs/code && uv run pytest tests/unit_tests/niki/test_perf_probes.py -q',
  },
  {
    id: 'memory',
    badge: 'Bounded Window',
    headline: 'Hour two costs what minute one cost',
    metric: 'bounded',
    before: 'Unbounded widget tree',
    after: 'Bounded mount window',
    beforePct: 100,
    afterPct: 64,
    method:
      'Offloaded turns are serialized to the session database instead of staying mounted, so the widget tree does not grow with transcript length.',
    verification:
      'Loading thousands of stored messages is asserted against an RSS growth ceiling rather than trended by eye.',
    command:
      'cd libs/code && uv run pytest tests/unit_tests/niki/test_perf_probes.py -q -k scroll',
  },
  {
    id: 'contrast',
    badge: 'WCAG Contrast',
    headline: 'All four runtime themes clear the contrast floors in every colour depth',
    metric: '4 themes tested',
    before: 'Unverified terminal hues',
    after: 'Floors asserted in tests',
    beforePct: 45,
    afterPct: 100,
    method:
      'Automated tests assert at least 4.5:1 for text and 3:1 for UI glyphs against every surface, in truecolor, 256-colour, and 16-colour terminals.',
    verification:
      'The palettes are measured, not eyeballed: a palette that drops below a floor fails the suite.',
    command: 'cd libs/code && uv run pytest tests/unit_tests/niki/test_theme_contrast.py -q',
  },
  {
    id: 'keymap',
    badge: 'Keymap Registry',
    headline: 'Every advertised shortcut is generated from a live binding',
    metric: 'no dead keys',
    before: 'Static docs drifting from bindings',
    after: 'Single live binding registry',
    beforePct: 62,
    afterPct: 100,
    method:
      'KEYMAP.md, the footer hints, and --help all render from libs/code/niki_agent/niki/keymap.py.',
    verification:
      'test_every_advertised_action_has_a_handler fails the build if any documented shortcut lacks a live action handler.',
    command: 'cd libs/code && uv run python scripts/gen_keymap.py',
  },
];

export const FEATURE_DETAILS: Record<string, FeatureDetail> = {
  install: {
    id: 'install',
    name: 'Install in one line',
    category: 'Presentation Layer',
    tagline: 'One installer line, checksum verified, no API key required to look around.',
    description:
      'The installer creates an isolated environment under ~/.local/niki, verifies the release checksum, and puts niki on your PATH. It never edits your shell profile unless you ask with NIKI_SHELL=1.',
    capabilities: [
      'Linux and macOS: curl -fsSL …/install/install.sh | bash',
      'Windows: irm …/install/install.ps1 | iex in PowerShell',
      'First launch asks your name and leaves you at a working composer',
      'Add a provider key from /auth or an environment variable when you need it',
    ],
    codePreview: `${INSTALL_COMMAND}\nniki`,
  },
  independence: {
    id: 'independence',
    name: 'One Package, One Command',
    category: 'Architecture',
    tagline: 'The agent loop lives in this repository; nothing else has to be installed.',
    description:
      'The runtime, the tool registry, the approval policy, and the editor-protocol integration are implemented here, and the vendored SDK ships inside the same package. Attribution for included components is kept in NOTICE.',
    capabilities: [
      'Distribution niki-agent, package niki_agent, command niki',
      'State directory ~/.niki, moved as a whole with NIKI_HOME',
      'A project can carry its own .niki/ directory for skills, hooks, and MCP',
      'Every NIKI_* variable means the same thing on Linux, macOS, and Windows',
    ],
  },
  'agent-loop': {
    id: 'agent-loop',
    name: 'One Agent Loop',
    category: 'Architecture',
    tagline: 'A real approval gate in front of every tool call.',
    description:
      'Niki reads the repository, edits files, runs commands, and shows you each step. Shell commands, writes, deletes, web access, and subagent delegation pass through an approval policy you control.',
    capabilities: [
      'Manual, Auto, and YOLO modes cycle on Shift+Tab; the status bar names the active mode',
      '1 / 2 / 3 select an approval option, a approves with auto, n rejects',
      'ctrl+t toggles the subagent panel and ctrl+o toggles inline tool output',
      'Auto and YOLO are policy, not a sandbox: use --sandbox for a hard boundary',
    ],
  },
  baseline: {
    id: 'baseline',
    name: 'Stillness at Rest',
    category: 'Presentation Layer',
    tagline: 'An idle session stops painting until you type.',
    description:
      'The composer owns focus, so a blinking cursor would repaint the terminal forever. Niki disarms the blink and the status timers on input idle, and the perf probe asserts the resting window paints nothing.',
    capabilities: [
      'The probe counts the compositor\u2019s own repaints, so a false pass is impossible',
      'The phase indicator only sweeps while the agent is working or streaming',
      'NIKI_REDUCED_MOTION=1 disables animation outright',
      'Resuming work re-arms the timers without a redraw storm',
    ],
    codePreview: 'cd libs/code && uv run pytest tests/unit_tests/niki/test_perf_probes.py -q',
  },
  'adaptive-motion': {
    id: 'adaptive-motion',
    name: 'Adaptive Phase Motion',
    category: 'Presentation Layer',
    tagline: 'A ping-ponging ◐ ◓ ◑ ◒ sweep that respects stillness.',
    description:
      'Motion only plays while the agent is actively thinking or streaming. At rest the terminal produces no frame invalidations, and reduced motion is honoured from the environment.',
    capabilities: [
      'Ping-ponging phase indicator with adaptive cadence',
      'NIKI_REDUCED_MOTION=1 disables all animation cleanly',
      'NIKI_UI_CHARSET_MODE=ascii and NO_COLOR=1 support minimal shells',
      'NIKI_SCREEN_READER=1 switches to linear, append-only output',
    ],
  },
  'themes-system': {
    id: 'themes-system',
    name: 'Runtime Theme Registry',
    category: 'Presentation Layer',
    tagline: 'Four palettes registered from a single module, measured by tests.',
    description:
      'Switch palettes live with /theme without restarting your session. Every foreground/background pair is measured in truecolor, 256-colour, and 16-colour terminals.',
    capabilities: [
      'niki — cool flat greys with a single focused accent',
      'niki-light — crisp light terminals',
      'niki-contrast — for bright rooms and low-quality panels',
      'niki-dim — lower luminance for dark rooms',
    ],
  },
  keymap: {
    id: 'keymap',
    name: 'Live Keymap Registry',
    category: 'Architecture',
    tagline: 'One source of truth powering --help, footer hints, and KEYMAP.md.',
    description:
      'Generated directly from libs/code/niki_agent/niki/keymap.py so no UI surface can ever advertise a dead keybinding.',
    capabilities: [
      '1 / 2 / 3 / a — pick an approval option or approve with auto',
      'Shift+Tab — cycle Manual → Auto → YOLO approval modes',
      'Ctrl+Shift+P — open the searchable command palette',
      'ctrl+o toggles tool output, ctrl+t toggles the subagent panel, ctrl+g opens your editor',
    ],
  },
  'ci-tests': {
    id: 'ci-tests',
    name: 'Hermetic Test Suite',
    category: 'Architecture',
    tagline: 'No network, no API key, no TTY required to run the tests.',
    description:
      'The suite runs locally in seconds and covers contrast math, keymap bindings, mount windows, and the first frame, so a claim on this page can be checked instead of trusted.',
    capabilities: [
      'test_every_advertised_action_has_a_handler prevents dead shortcuts',
      'Contrast floors asserted for text and UI glyphs in every colour depth',
      'An idle-session probe that counts real compositor repaints',
      'Snapshot tests check layout at 40, 50, 80, 120, and 160 columns',
    ],
    codePreview: 'cd libs/code\nuv run pytest tests/unit_tests/niki/ -q',
  },
  'mount-window': {
    id: 'mount-window',
    name: 'Bounded Mount Window',
    category: 'Architecture',
    tagline: 'A fixed visible window instead of an ever-growing widget tree.',
    description:
      'Rather than mounting every historical turn forever, the message store keeps a bounded visible window and prunes the turns below it, keeping frame time flat in a long session.',
    capabilities: [
      'Offscreen turns stay out of the mounted tree',
      'Full scrollback and keyboard navigation (pageup/pagedown/home/end) keep working',
      'Scroll paint time and RSS growth are asserted on a 5,000-message transcript',
      'The store still resolves any stored message by id',
    ],
  },
  providers: {
    id: 'providers',
    name: 'Providers',
    category: 'Providers & Docs',
    tagline: 'Three providers built in, eighteen extras, and any OpenAI-compatible endpoint.',
    description:
      'Anthropic, OpenAI (including Azure OpenAI and ChatGPT subscription login), and Google Gemini work out of the box. Everything else is one /install away, and a gateway is a config block in ~/.niki/config.toml.',
    capabilities: [
      'Install an integration from inside a session: /install ollama',
      'Keys come from /auth or the vendor environment variable; /auth stores them 0600',
      'Ollama needs no key: Niki discovers the models you have pulled',
      'Full gateway notes live in docs/niki/PROVIDERS.md',
    ],
  },
};

export const ARCHITECTURE_NAV_GROUPS = [
  {
    heading: 'Presentation Layer',
    items: [
      { label: 'Install', desc: 'One installer line, then niki', detailId: 'install' },
      { label: 'Stillness at Rest', desc: 'Timers disarm when the agent is idle', detailId: 'baseline' },
      { label: 'Runtime Themes', desc: '4 palettes via /theme', detailId: 'themes-system' },
      { label: 'Adaptive Motion', desc: '◐ ◓ ◑ ◒ cadence + reduced motion support', detailId: 'adaptive-motion' },
      { label: 'Bounded Mount Window', desc: 'Keep long transcripts lean and fast', detailId: 'mount-window' },
      { label: 'Live Keymap', desc: 'No dead shortcuts, checked in CI', detailId: 'keymap' },
    ],
  },
  {
    heading: 'Foundation & Providers',
    items: [
      { label: 'Providers', desc: '3 built in, 18 extras, any gateway', detailId: 'providers' },
      { label: 'One Agent Loop', desc: 'Tools, approvals, and subagents', detailId: 'agent-loop' },
      { label: 'One Package', desc: 'One command, one state directory', detailId: 'independence' },
    ],
  },
];

export const DOCS_NAV_ITEMS = [
  { label: 'ARCHITECTURE.md', desc: 'How the agent, the UI, and the tools fit together', href: '#bento-features' },
  { label: 'INDEPENDENCE.md', desc: 'Package, state, and environment guarantees', href: '#parallax-story' },
  { label: 'KEYMAP.md', desc: 'Generated directly from the live binding registry', href: '#bento-features' },
  { label: 'PROVIDERS.md', desc: 'Provider setup and gateway notes', href: '#hero' },
  { label: 'SECURITY.md', desc: 'Reporting a vulnerability and what is in scope', href: '#stats' },
  { label: 'NOTICE', desc: 'Third-party copyright notices', href: '#get-started' },
];

export const MEGA_MENU_PROVIDER_PAIRINGS = [
  {
    id: 'anthropic',
    label: 'Niki + Anthropic',
    command: 'export ANTHROPIC_API_KEY=sk-ant-... && niki',
  },
  {
    id: 'openai',
    label: 'Niki + OpenAI',
    command: 'export OPENAI_API_KEY=sk-... && niki',
  },
  {
    id: 'google',
    label: 'Niki + Google Gemini',
    command: 'export GOOGLE_API_KEY=... && niki',
  },
  {
    id: 'ollama',
    label: 'Niki + Ollama Local',
    command: '/install ollama',
  },
  {
    id: 'openrouter',
    label: 'Niki + OpenRouter',
    command: '/install openrouter',
  },
  {
    id: 'nvidia',
    label: 'Niki + NVIDIA NIM',
    command: '/install nvidia',
  },
  {
    id: 'deepseek',
    label: 'Niki + DeepSeek',
    command: '/install deepseek',
  },
  {
    id: 'kimi',
    label: 'Niki + Kimi TOML',
    command: '[models.providers.kimi]',
  },
  {
    id: 'kilo',
    label: 'Niki + Kilo Gateway',
    command: '[models.providers.kilo]',
  },
];

export const MEGA_MENU_TOP_PROVIDERS = [
  { id: 'anthropic', name: 'Anthropic' },
  { id: 'openai', name: 'OpenAI' },
  { id: 'google', name: 'Google' },
  { id: 'ollama', name: 'Ollama' },
  { id: 'openrouter', name: 'OpenRouter' },
  { id: 'nvidia', name: 'NVIDIA' },
  { id: 'groq', name: 'Groq' },
  { id: 'together', name: 'Together AI' },
  { id: 'deepseek', name: 'DeepSeek' },
  { id: 'kimi', name: 'Kimi' },
];

export const MEGA_MENU_CAPABILITIES = [
  {
    id: 'adaptive-motion',
    icon: 'motion' as const,
    title: 'Adaptive Motion',
    desc: 'No idle redraws at rest; sweeps only while streaming.',
    detailId: 'adaptive-motion',
  },
  {
    id: 'install',
    icon: 'terminal' as const,
    title: 'One-Line Install',
    desc: 'Installer line, then a working composer.',
    detailId: 'install',
  },
  {
    id: 'themes-system',
    icon: 'shield' as const,
    title: 'WCAG Contrast',
    desc: 'Four themes, floors asserted by tests.',
    detailId: 'themes-system',
  },
  {
    id: 'independence',
    icon: 'layers' as const,
    title: 'One Package',
    desc: 'The agent loop ships in this repository.',
    detailId: 'independence',
  },
];

export const MEGA_MENU_LEVELS = [
  {
    level: '01',
    title: 'Core Surface Bento',
    desc: 'Composer, install, providers & live keymap.',
    href: '#bento-features',
  },
  {
    level: '02',
    title: 'Scroll Parallax Craft',
    desc: 'Stillness at rest, bounded mount window, & shell accessibility.',
    href: '#parallax-story',
  },
  {
    level: '03',
    title: '6-Stage Orbital Dial',
    desc: 'Interactive Boot → Still → Theme → Bind → Scale → Route view.',
    href: '#lifecycle',
  },
  {
    level: '04',
    title: 'Verified Baselines',
    desc: 'Stillness, mount window, contrast, and keymap proofs.',
    href: '#benchmarks',
  },
];

export const MEGA_MENU_GUIDES = [
  {
    id: 'guide-install',
    badge: 'INSTALL',
    title: 'Install Niki Agent in one line',
    terminalCmd: '$ curl -fsSL …/install/install.sh | bash',
    terminalOut: '◐ Niki Agent · composer ready',
    detailId: 'install',
  },
  {
    id: 'guide-toml',
    badge: 'ZERO-DEP TOML',
    title: 'Wire Kimi, Kilo & OpenCode in config.toml',
    terminalCmd: '[models.providers.kimi]',
    terminalOut: 'class_path = "langchain_openai:ChatOpenAI"',
    detailId: 'providers',
  },
];

export const MEGA_MENU_BASELINES = [
  {
    id: 'idle-redraws',
    metric: 'still',
    title: 'Idle Stillness',
    desc: 'An idle session paints nothing; the probe asserts it.',
    detailId: 'baseline',
  },
  {
    id: 'runtime-themes',
    metric: '4',
    title: 'Runtime Themes',
    desc: 'Every palette measured at its text and glyph floors.',
    detailId: 'themes-system',
  },
];

export const FOOTER_COLUMNS = {
  product: [
    { label: 'Install', detailId: 'install' },
    { label: 'Stillness at Rest', detailId: 'baseline' },
    { label: 'Runtime Themes (/theme)', detailId: 'themes-system' },
    { label: 'Adaptive Motion', detailId: 'adaptive-motion' },
    { label: 'Bounded Mount Window', detailId: 'mount-window' },
    { label: 'Live Keymap Registry', detailId: 'keymap' },
    { label: 'Providers', detailId: 'providers' },
    { label: 'Test Suite', detailId: 'ci-tests' },
  ],
  docs: [
    { label: 'Documentation site', href: 'https://niki-docs.vercel.app' },
    { label: 'README.md', href: 'https://github.com/RavaniRoshan/niki-agent' },
    { label: 'ARCHITECTURE.md', href: 'https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/docs/niki/ARCHITECTURE.md' },
    { label: 'INDEPENDENCE.md', href: 'https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/docs/niki/INDEPENDENCE.md' },
    { label: 'KEYMAP.md', href: 'https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/docs/niki/KEYMAP.md' },
    { label: 'PROVIDERS.md', href: 'https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/docs/niki/PROVIDERS.md' },
    { label: 'SECURITY.md', href: 'https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/SECURITY.md' },
    { label: 'NOTICE', href: 'https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/NOTICE' },
  ],
  ecosystem: [
    { label: 'GitHub Repository', href: 'https://github.com/RavaniRoshan/niki-agent' },
    { label: 'Textual 8', href: 'https://textual.textualize.io' },
    { label: 'Astral uv', href: 'https://docs.astral.sh/uv/' },
    { label: 'Python 3.12+', href: 'https://www.python.org' },
    { label: 'MIT NOTICE', href: 'https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/NOTICE' },
  ],
};
