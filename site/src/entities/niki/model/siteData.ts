export interface EcosystemLogoItem {
  id: string;
  name: string;
  actionType?: 'Built-in' | 'Config';
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
  id: 'binary' | 'install' | 'sessions' | 'contrast' | 'keymap';
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
  id: 'niki' | 'high-contrast' | 'colorblind' | 'mono';
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
  id: 'binary' | 'sessions' | 'accessibility' | 'approvals';
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

export const WINDOWS_INSTALL_COMMAND =
  'irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex';


export const TEST_COMMAND = 'cargo test -j 4 --workspace';

const SLASH_COMMANDS = [
  '/auto',
  '/clear',
  '/compact',
  '/context',
  '/diff',
  '/export',
  '/help',
  '/init',
  '/manual',
  '/model',
  '/quit',
  '/rename',
  '/resume',
  '/skills',
  '/theme',
  '/threads',
  '/tools',
  '/version',
  '/yolo',
] as const;

const SLASH_COMMAND_LINE = `Slash commands: ${SLASH_COMMANDS.join(', ')}`;

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
    command: 'ollama serve && niki',
    placeholder: 'ollama serve && niki',
  },
];

export const PARALLAX_CHAPTERS: ParallaxChapter[] = [
  {
    id: 'binary',
    indexLabel: '01',
    title: 'One binary, no runtime',
    summary:
      'The turn loop, tool registry, approval policy, provider adapters, session store, MCP client, and terminal UI are Rust crates in this repository, linked into one executable. Nothing else has to be installed to run it.',
    metricValue: '1 binary',
    metricLabel: 'self-contained · no runtime',
    detailId: 'independence',
  },
  {
    id: 'sessions',
    indexLabel: '02',
    title: 'Sessions that survive',
    summary:
      'Every turn is written to a local SQLite store under ~/.niki. /threads browses saved sessions and previews their first prompt, /resume brings one back, /compact folds older turns into a summary to keep the conversation inside the context window, and /clear starts a fresh one. From the shell, `niki sessions fork` copies a session and `archive` hides one from every picker while an explicit id still opens it.',
    metricValue: 'SQLite',
    metricLabel: 'local session store',
    detailId: 'sessions',
  },
  {
    id: 'accessibility',
    indexLabel: '03',
    title: 'Themes measured, not eyeballed',
    summary:
      'niki, high-contrast, colorblind, and mono are plain sRGB palettes. A test computes WCAG ratios for every text, muted, accent, error, ok, and border pair, and NIKI_REDUCED_MOTION=1 stops the spinner.',
    metricValue: '4 themes',
    metricLabel: 'contrast gate in cargo test',
    detailId: 'themes-system',
  },
  {
    id: 'approvals',
    indexLabel: '04',
    title: 'A gate in front of every tool',
    summary:
      'Manual, Auto, and YOLO modes cycle on Shift+Tab. Auto skips the prompt for commands on your shell allow-list and for file writes whose target stays inside the workspace — there is no classifier — and a workspace path boundary refuses anything outside the root. A repository\'s own hooks, skills and instruction files do not load until `niki trust` records that workspace.',
    metricValue: '3 modes',
    metricLabel: 'manual · auto · yolo',
    detailId: 'agent-loop',
  },
];

export const NIKI_THEMES: NikiThemePalette[] = [
  {
    id: 'niki',
    name: 'niki',
    description: 'The default dark palette. Cool greys, one accent.',
    tag: 'DEFAULT',
    bg: '#101014',
    surface: '#1b1c22',
    border: '#6b7280',
    text: '#e6e7ec',
    muted: '#aab0bd',
    accent: '#8ec7ff',
  },
  {
    id: 'high-contrast',
    name: 'high-contrast',
    description: 'Light and high contrast for bright rooms.',
    tag: 'LIGHT',
    bg: '#ffffff',
    surface: '#eeeeee',
    border: '#5f5f5f',
    text: '#101010',
    muted: '#3d3d3d',
    accent: '#003a8c',
  },
  {
    id: 'colorblind',
    name: 'colorblind',
    description: 'Okabe–Ito hues for red/green viewers.',
    tag: 'SAFE',
    bg: '#16171d',
    surface: '#21232b',
    border: '#6f7a86',
    text: '#eceff4',
    muted: '#b6bac6',
    accent: '#56b4e9',
  },
  {
    id: 'mono',
    name: 'mono',
    description: 'Greys only; meaning carried by weight and markers.',
    tag: 'MONO',
    bg: '#0b0b0b',
    surface: '#1a1a1a',
    border: '#707070',
    text: '#f2f2f2',
    muted: '#b3b3b3',
    accent: '#ffffff',
  },
];

export const ECOSYSTEM_LOGOS_ROW_1: EcosystemLogoItem[] = [
  {
    id: 'anthropic',
    name: 'Anthropic',
    actionType: 'Built-in',
    envVar: 'ANTHROPIC_API_KEY',
    summary: 'Built in. Set a key and pick an Anthropic model in /model; nothing else to install.',
    configSnippet: `export ANTHROPIC_API_KEY=sk-ant-...\nniki`,
  },
  {
    id: 'openai',
    name: 'OpenAI',
    actionType: 'Built-in',
    envVar: 'OPENAI_API_KEY',
    summary: 'Built in. Set a key and pick a model in /model.',
    configSnippet: `export OPENAI_API_KEY=sk-...\nniki`,
  },
  {
    id: 'google',
    name: 'Google',
    actionType: 'Built-in',
    envVar: 'GOOGLE_API_KEY',
    summary: 'Gemini models speak the OpenAI-compatible surface; one key variable is all they need.',
    configSnippet: `export GOOGLE_API_KEY=...\nniki`,
  },
  {
    id: 'ollama',
    name: 'Ollama',
    actionType: 'Built-in',
    envVar: 'OLLAMA_HOST',
    summary: 'Local models, keyless. Start the daemon and /model lists what you have pulled.',
    configSnippet: `ollama serve\n/model`,
  },
  {
    id: 'openrouter',
    name: 'OpenRouter',
    actionType: 'Built-in',
    envVar: 'OPENROUTER_API_KEY',
    summary: 'One built-in adapter, hundreds of routed models.',
    configSnippet: `export OPENROUTER_API_KEY=sk-or-...\nniki`,
  },
  {
    id: 'nvidia',
    name: 'NVIDIA',
    actionType: 'Built-in',
    envVar: 'NVIDIA_API_KEY',
    summary: 'NVIDIA NIM endpoints are built in.',
    configSnippet: `export NVIDIA_API_KEY=nvapi-...\nniki`,
  },
  {
    id: 'groq',
    name: 'Groq',
    actionType: 'Built-in',
    envVar: 'GROQ_API_KEY',
    summary: 'Low-latency LPU inference is built in.',
    configSnippet: `export GROQ_API_KEY=gsk_...\nniki`,
  },
  {
    id: 'together',
    name: 'Together',
    actionType: 'Built-in',
    envVar: 'TOGETHER_API_KEY',
    summary: 'Open-weights coding models are built in.',
    configSnippet: `export TOGETHER_API_KEY=...\nniki`,
  },
  {
    id: 'deepseek',
    name: 'DeepSeek',
    actionType: 'Built-in',
    envVar: 'DEEPSEEK_API_KEY',
    summary: 'DeepSeek reasoning and coding models are built in.',
    configSnippet: `export DEEPSEEK_API_KEY=...\nniki`,
  },
  {
    id: 'mistral',
    name: 'Mistral',
    actionType: 'Built-in',
    envVar: 'MISTRAL_API_KEY',
    summary: 'Mistral models are built in.',
    configSnippet: `export MISTRAL_API_KEY=...\nniki`,
  },
];

export const ECOSYSTEM_LOGOS_ROW_2: EcosystemLogoItem[] = [
  {
    id: 'xai',
    name: 'xAI',
    actionType: 'Built-in',
    envVar: 'XAI_API_KEY',
    summary: 'xAI models are built in.',
    configSnippet: `export XAI_API_KEY=...\nniki`,
  },
  { id: 'rust', name: 'Rust' },
  { id: 'cargo', name: 'Cargo' },
  { id: 'ratatui', name: 'ratatui' },
  { id: 'sqlite', name: 'SQLite' },
  { id: 'mcp', name: 'MCP' },
  { id: 'github', name: 'GitHub' },
];

export const HERO_FEATURED_LOGOS: EcosystemLogoItem[] = [
  ECOSYSTEM_LOGOS_ROW_1[0], // Anthropic
  ECOSYSTEM_LOGOS_ROW_1[1], // OpenAI
  ECOSYSTEM_LOGOS_ROW_1[2], // Google
  ECOSYSTEM_LOGOS_ROW_1[3], // Ollama
  ECOSYSTEM_LOGOS_ROW_1[4], // OpenRouter
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
      { label: 'One Binary', detailId: 'independence' },
      { label: 'Agent Loop', detailId: 'agent-loop' },
    ],
  },
  {
    id: 'still',
    label: 'Binary',
    headline: 'One self-contained executable, with no runtime to install.',
    bullets: [
      { lead: 'Ship', text: 'a Rust binary instead of an interpreter and a package manager' },
      { lead: 'Link', text: 'the turn loop, tools, policy, sessions, MCP, and TUI from this workspace' },
      { lead: 'Verify', text: 'SHA256SUMS before the installer places the binary' },
    ],
    links: [
      { label: 'One Binary', detailId: 'independence' },
      { label: 'Test Suite', detailId: 'ci-tests' },
    ],
  },
  {
    id: 'theme',
    label: 'Theme',
    headline: 'Four palettes with contrast checked by the test suite.',
    bullets: [
      { lead: 'Switch', text: 'live mid-session between niki, high-contrast, colorblind, and mono' },
      { lead: 'Measure', text: 'text at 4.5:1 and UI glyphs at 3:1 against every surface' },
      { lead: 'Read', text: 'clearly in bright rooms with high-contrast' },
    ],
    links: [
      { label: 'Theme Registry', detailId: 'themes-system' },
      { label: 'Contrast Tests', detailId: 'ci-tests' },
    ],
  },
  {
    id: 'bind',
    label: 'Keys',
    headline: 'Keyboard-native control generated from one binding registry.',
    bullets: [
      { lead: 'Approve', text: 'a tool call with y and reject it with n' },
      { lead: 'Cycle', text: 'Manual → Auto → YOLO approval modes with Shift+Tab' },
      { lead: 'Steer', text: 'a running turn with Alt+Enter; Enter queues the next prompt and the status bar counts it' },
      { lead: 'Open', text: 'the searchable command palette with Ctrl+Shift+P' },
    ],
    links: [
      { label: 'Live Keymap', detailId: 'keymap' },
      { label: 'Approval Modes', detailId: 'agent-loop' },
    ],
  },
  {
    id: 'scale',
    label: 'Sessions',
    headline: 'Sessions persist in a local SQLite store.',
    bullets: [
      { lead: 'Browse', text: 'saved sessions with /threads, or resume one by id or index with /resume' },
      { lead: 'Fold', text: 'older turns into a summary with /compact; the store keeps them' },
      { lead: 'Clear', text: 'the transcript and start fresh with /clear' },
    ],
    links: [
      { label: 'Session Store', detailId: 'sessions' },
      { label: 'Live Composer', detailId: 'adaptive-motion' },
    ],
  },
  {
    id: 'route',
    label: 'Route',
    headline: 'Eleven providers built in, any OpenAI-compatible endpoint one block away.',
    bullets: [
      { lead: 'Connect', text: 'Anthropic, OpenAI, Google Gemini, Groq, OpenRouter, Together, DeepSeek, NVIDIA, Mistral, xAI, and Ollama' },
      { lead: 'Switch', text: 'models with /model without restarting the session' },
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
    title: 'Single binary, single-rule composer',
    subtitle: 'Bracketed paste, queued turns, and no runtime to install',
    ctaLabel: 'Explore the composer',
    installCmd: INSTALL_COMMAND,
    highlights: [
      'One Rust executable: no interpreter, package manager, or virtual environment',
      'Bracketed paste keeps multi-line text intact; one hairline composer rule',
      'Enter while a turn runs queues the prompt in a dim queued row; the status bar counts it',
      'Alt+Enter or Ctrl+Enter steers the turn that is already running',
      'Esc twice on an empty composer restores the last prompt and drops its answer from the view',
      '! runs a line in the local shell through the same approval policy, without a model turn',
      'Name a saved session with /rename and write it to Markdown with /export',
      SLASH_COMMAND_LINE,
    ],
    codeSnippet: `# Interactive terminal session (no API key needed to look around)
niki

# One prompt, then exit
niki -n "explain the workspace layout"

# The same turn as one JSON event per line, for scripts
niki -n "explain the workspace layout" --output-format jsonl

# Run the workspace suite without a network or TTY
cargo test -j 4 --workspace`,
  },
  {
    id: 'niki-themes',
    packageName: 'niki · themes',
    title: 'Four palettes switchable at runtime',
    subtitle: 'Contrast ratios asserted by cargo test',
    ctaLabel: 'Explore themes',
    installCmd: '/theme high-contrast',
    highlights: [
      'niki, high-contrast, colorblind, and mono',
      'WCAG 4.5:1 text and 3:1 UI glyph floors computed in the test suite',
      'NIKI_REDUCED_MOTION=1 stops the spinner without changing the layout',
    ],
    codeSnippet: `# Switch runtime palettes live inside Niki Agent
/theme niki            # cool greys, one accent
/theme high-contrast   # light, maximum separation
/theme colorblind      # Okabe–Ito hues
/theme mono            # greys only, meaning by weight`,
  },
  {
    id: 'niki-providers',
    packageName: 'niki · providers',
    title: 'Eleven providers built in',
    subtitle: 'Any OpenAI-compatible gateway is one TOML block away',
    ctaLabel: 'Explore providers',
    installCmd: INSTALL_COMMAND,
    highlights: [
      'Built in: Anthropic, OpenAI, Google Gemini, Groq, OpenRouter, Together, DeepSeek, NVIDIA, Mistral, xAI, and Ollama',
      'Ollama is keyless and talks to your local daemon',
      'Any OpenAI-compatible endpoint is a block in ~/.niki/config.toml',
    ],
    codeSnippet: `# ~/.niki/config.toml — no new runtime dependency
[providers.kimi]
display_name = "Kimi"
base_url = "https://api.moonshot.ai/v1"
api_key_env = "MOONSHOT_API_KEY"
protocol = "openai"
models = ["kimi-k3", "kimi-k2.7-code", "kimi-k2.6"]`,
  },
];

export const BENCHMARK_STORIES: BenchmarkStory[] = [
  {
    id: 'binary',
    badge: 'No Runtime',
    headline: 'The product is one self-contained binary',
    metric: '1 binary',
    before: 'Runtime plus package manager',
    after: 'One Rust executable',
    beforePct: 100,
    afterPct: 12,
    method:
      'The workspace links the turn loop, tool registry, approval policy, provider adapters, session store, MCP client, skills, hooks, and terminal UI into a single niki executable.',
    verification:
      'The release build is cargo build --release -p niki --bin niki. No interpreter or virtual environment is installed beside it.',
    command: 'cargo build --release -p niki --bin niki',
  },
  {
    id: 'install',
    badge: 'One-Line Install',
    headline: 'One installer line replaces a toolchain setup',
    metric: '1 line',
    before: 'Toolchain setup',
    after: 'Install script',
    beforePct: 100,
    afterPct: 20,
    method:
      'install/install.sh on Linux and WSL and install/install.ps1 on Windows fetch the release archive, verify the checksum file, and install the niki binary.',
    verification:
      'The SHA256SUMS check is part of the install path; a mismatch stops the script before anything is installed.',
    command: INSTALL_COMMAND,
  },
  {
    id: 'sessions',
    badge: 'Session Store',
    headline: 'Resume a session instead of recreating it',
    metric: 'SQLite',
    before: 'Lost on exit',
    after: '/resume',
    beforePct: 100,
    afterPct: 8,
    method:
      'Turns are written to a local SQLite session store under ~/.niki. /threads browses them and previews each first prompt, /resume lists and restores one, /compact folds older turns into a summary, and /clear starts a fresh session.',
    verification:
      'Sessions live beside config, skills, hooks, and MCP settings, so NIKI_HOME moves the whole tree together.',
    command: 'niki -n "summarise this workspace"',
  },
  {
    id: 'contrast',
    badge: 'WCAG Contrast',
    headline: 'All four palettes clear the contrast floors',
    metric: '4 themes',
    before: 'Unverified terminal hues',
    after: 'Floors asserted in tests',
    beforePct: 45,
    afterPct: 100,
    method:
      'The theme crate computes WCAG relative luminance for every text, muted, accent, error, ok, and border pair in niki, high-contrast, colorblind, and mono.',
    verification:
      'A test asserts at least 4.5:1 for text and 3:1 for UI glyphs; a palette that drops below a floor fails the suite.',
    command: 'cargo test -j 4 -p niki-tui-themes',
  },
  {
    id: 'keymap',
    badge: 'Binding Registry',
    headline: 'Every advertised shortcut is a bound action',
    metric: 'no dead keys',
    before: 'Docs drifting from bindings',
    after: 'Single live binding registry',
    beforePct: 62,
    afterPct: 100,
    method:
      'One table in crates/niki-tui backs ctrl+shift+p, Shift+Tab, y and n approvals, ?, and the scroll keys.',
    verification:
      'A test parses every advertised chord label back to its action, so a documented key cannot drift from its handler.',
    command: 'cargo test -j 4 -p niki-tui',
  },
];

export const FEATURE_DETAILS: Record<string, FeatureDetail> = {
  install: {
    id: 'install',
    name: 'Install in one line',
    category: 'Architecture',
    tagline: 'One installer line, checksum verified, no API key required to look around.',
    description:
      'The installer verifies the release against SHA256SUMS, installs the self-contained niki binary, and leaves you at a working composer. No runtime, package manager, or virtual environment is created.',
    capabilities: [
      'Linux and WSL: curl -fsSL …/install/install.sh | bash',
      'Windows: irm …/install/install.ps1 | iex in PowerShell',
      'macOS arm64 and x86_64: the same one-line installer',
      'Add a provider key from an environment variable when you need it',
    ],
    codePreview: `${INSTALL_COMMAND}\nniki`,
  },
  independence: {
    id: 'independence',
    name: 'One Binary, One Command',
    category: 'Architecture',
    tagline: 'The agent loop lives in this repository and links into one executable.',
    description:
      'The turn loop, tool registry, approval policy, provider adapters, session store, MCP client, skills, hooks, slash commands, and terminal UI are workspace crates. Nothing imports an external runtime; the release build produces a single niki binary.',
    capabilities: [
      'Crates: niki-config, niki-provider, niki-agent, niki-tools, niki-policy, niki-workspace, niki-session, niki-mcp',
      'Crates: niki-skills, niki-hooks, niki-commands, niki-tui, niki-tui-live, niki-tui-modals, niki-tui-themes, and the niki binary',
      'State directory ~/.niki, moved as a whole with NIKI_HOME',
      'Skills can be project-scoped or user-scoped',
      'MCP servers run over stdio and HTTP',
      'Lifecycle hooks: PreToolUse, PostToolUse, and UserPromptSubmit',
      '[notify] runs a program once when a turn ends, with a JSON event on its stdin',
    ],
  },
  'agent-loop': {
    id: 'agent-loop',
    name: 'One Agent Loop',
    category: 'Architecture',
    tagline: 'A real approval gate in front of every tool call.',
    description:
      'Niki reads the repository, edits files, runs commands, and shows you each step. Shell commands, writes, deletes, and MCP tools pass through an approval policy you control.',
    capabilities: [
      'Manual, Auto, and YOLO modes cycle on Shift+Tab; the status bar names the active mode',
      'Auto skips the prompt for allow-listed commands and for writes that stay inside the workspace — there is no classifier',
      'write_file and edit_file ask first; Auto runs them unattended only when the path stays inside the workspace',
      'A workspace path boundary refuses any path that resolves outside the root',
      'AGENTS.md is read from the workspace root down to the working directory, user-level file first, inside one 32 KiB budget',
      '/init writes an AGENTS.md template when the workspace has none',
      'y or a allows once, s allows for the session, d or n refuses; Shift+Tab cycles the mode',
    ],
  },
  binary: {
    id: 'binary',
    name: 'One Self-Contained Binary',
    category: 'Architecture',
    tagline: 'No interpreter, no package manager, no virtual environment.',
    description:
      'The release profile links the whole agent into one executable. The Linux and WSL installer, the macOS installer, and the Windows installer all deliver the same binary shape; the installer verifies SHA256SUMS first.',
    capabilities: [
      'Linux and WSL: curl -fsSL …/install/install.sh | bash',
      'Windows PowerShell: irm …/install/install.ps1 | iex',
      'macOS arm64 and x86_64: curl -fsSL …/install/install.sh | bash',
      'The same binary serves the full-screen app and headless niki -n',
    ],
    codePreview: 'cargo build --release -p niki --bin niki',
  },
  'adaptive-motion': {
    id: 'adaptive-motion',
    name: 'Live Composer & Bracketed Paste',
    category: 'Presentation Layer',
    tagline: 'Paste multi-line text as text, not as keystrokes.',
    description:
      'Bracketed paste keeps newlines and punctuation inside the composer, and slash commands stay one word away. A prompt sent during a running turn waits in a dim queued row until that turn ends; Alt+Enter hands the draft to the running turn instead. Motion is a switch: the spinner runs while the agent works, and NIKI_REDUCED_MOTION=1 stops it.',
    capabilities: [
      'Bracketed paste sends a multi-line block as one text payload',
      'Enter queues a prompt while a turn runs; the status bar counts queued prompts',
      'Alt+Enter or Ctrl+Enter steers the turn that is already running',
      'Esc twice on an empty composer restores the last prompt and drops its answer from the view',
      '! runs a line in the local shell through the same approval policy, without a model turn',
      'NIKI_REDUCED_MOTION=1 disables the spinner',
      'NIKI_ASCII=1 falls back to an ASCII glyph and border set',
      SLASH_COMMAND_LINE,
    ],
  },
  'themes-system': {
    id: 'themes-system',
    name: 'Runtime Theme Registry',
    category: 'Presentation Layer',
    tagline: 'Four palettes from one module, measured by tests.',
    description:
      'Switch palettes live with /theme without restarting your session. Every foreground/background and border pair is measured by the contrast gate in the test suite.',
    capabilities: [
      'niki — the default dark palette',
      'high-contrast — light, maximum separation',
      'colorblind — Okabe–Ito hues on a dark ground',
      'mono — greys only; meaning carried by weight and markers',
    ],
  },
  keymap: {
    id: 'keymap',
    name: 'Live Keymap Registry',
    category: 'Architecture',
    tagline: 'One binding table behind the keys the help overlay advertises.',
    description:
      'crates/niki-tui holds the app-level binding table, and a test parses every advertised chord label back to its action so the help and the handlers cannot drift apart.',
    capabilities: [
      'y / a, s, d / n — allow once, allow for the session, or refuse',
      'Shift+Tab — cycle Manual → Auto → YOLO approval modes',
      'Ctrl+Shift+P — start a slash command in the composer',
      'Ctrl+T — the whole transcript, including rows that scrolled out of view',
      '? opens the help overlay; PageUp and PageDown move through the transcript',
    ],
  },
  'ci-tests': {
    id: 'ci-tests',
    name: 'Workspace Test Suite',
    category: 'Architecture',
    tagline: 'cargo test -j 4 --workspace, with no network or API key required.',
    description:
      'The workspace suite covers the contrast gate, keymap round-trips, the policy boundary, the session store, MCP transports, and committed replay fixtures, so a claim on this page can be checked instead of trusted.',
    capabilities: [
      'The contrast gate computes WCAG ratios for every palette pair',
      'Every advertised chord label parses back to its action',
      'The policy tests assert Manual, Auto, YOLO, and the workspace boundary',
      'Replay fixtures pin the rendered frame without an engine',
    ],
    codePreview: 'cargo test -j 4 --workspace',
  },
  sessions: {
    id: 'sessions',
    name: 'Session Store',
    category: 'Architecture',
    tagline: 'A local SQLite store instead of a transcript that disappears.',
    description:
      'Each turn is written to a session store under ~/.niki. /threads browses the saved sessions and previews their first prompt, /compact folds older turns into a summary with the model so the conversation stays inside the context window, and /clear starts a fresh session.',
    capabilities: [
      '/threads opens a session browser with a filter and a first-prompt preview',
      '/resume lists saved sessions or resumes one by id or index',
      '/compact replaces older turns with a model-written summary; the store keeps the originals',
      '/context reports the conversation size and what the session has spent',
      '/rename names the session; /export writes it to Markdown inside the workspace',
      '/clear empties the transcript and starts a new session',
      'Sessions sit beside config, skills, and hooks under NIKI_HOME',
    ],
  },
  'working-tree': {
    id: 'working-tree',
    name: 'Working-Tree Diff',
    category: 'Presentation Layer',
    tagline: 'Staged, unstaged, and untracked changes, drawn inside the transcript.',
    description:
      '/diff reads the working tree with git and renders staged and unstaged changes as unified diffs with old and new gutters; on the untracked side it reads each file and draws it as additions. Every section is capped so a large change cannot flood the transcript.',
    capabilities: [
      '/diff shows all three sections; /diff staged, /diff unstaged, and /diff untracked narrow it',
      'Each section stops at 20 files and 400 lines, and reports what it withheld',
      'An untracked file over 1 MiB is named with its size instead of being read',
      'A clean tree answers "no changes" without printing a diff',
    ],
    codePreview: '/diff unstaged',
  },
  providers: {
    id: 'providers',
    name: 'Providers',
    category: 'Providers & Docs',
    tagline: 'Eleven providers built in, and any OpenAI-compatible endpoint.',
    description:
      'Anthropic, OpenAI, Google Gemini, Groq, OpenRouter, Together, DeepSeek, NVIDIA, Mistral, xAI, and Ollama work out of the box. Everything else is a provider block in ~/.niki/config.toml.',
    capabilities: [
      'Built-in blocks carry the endpoint and the key environment variable',
      'Switch models with /model, or pass --model provider:model to niki -n',
      'Ollama needs no key: Niki talks to your local daemon',
      'Full gateway notes live in docs/niki/PROVIDERS.md',
    ],
  },
};

export const ARCHITECTURE_NAV_GROUPS = [
  {
    heading: 'Presentation Layer',
    items: [
      { label: 'Install', desc: 'One installer line, then niki', detailId: 'install' },
      { label: 'One Binary', desc: 'No runtime, no virtual environment', detailId: 'binary' },
      { label: 'Runtime Themes', desc: '4 palettes via /theme', detailId: 'themes-system' },
      { label: 'Live Composer', desc: 'Bracketed paste + reduced motion', detailId: 'adaptive-motion' },
      { label: 'Session Store', desc: 'SQLite sessions, /threads and /resume', detailId: 'sessions' },
      { label: 'Working-Tree Diff', desc: '/diff renders staged, unstaged, untracked', detailId: 'working-tree' },
      { label: 'Live Keymap', desc: 'Every advertised chord has a handler', detailId: 'keymap' },
    ],
  },
  {
    heading: 'Foundation & Providers',
    items: [
      { label: 'Providers', desc: '11 built in, any gateway', detailId: 'providers' },
      { label: 'One Agent Loop', desc: 'Tools, approvals, and MCP', detailId: 'agent-loop' },
      { label: 'One Command', desc: 'One executable, one state directory', detailId: 'independence' },
    ],
  },
];

export const DOCS_NAV_ITEMS = [
  { label: 'ARCHITECTURE.md', desc: 'How the agent, the UI, and the tools fit together', href: '#bento-features' },
  { label: 'INDEPENDENCE.md', desc: 'Binary, state, and environment guarantees', href: '#parallax-story' },
  { label: 'KEYMAP.md', desc: 'The binding table and the chords it advertises', href: '#bento-features' },
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
    command: 'ollama serve && niki',
  },
  {
    id: 'openrouter',
    label: 'Niki + OpenRouter',
    command: 'export OPENROUTER_API_KEY=sk-or-... && niki',
  },
  {
    id: 'nvidia',
    label: 'Niki + NVIDIA NIM',
    command: 'export NVIDIA_API_KEY=nvapi-... && niki',
  },
  {
    id: 'groq',
    label: 'Niki + Groq',
    command: 'export GROQ_API_KEY=gsk_... && niki',
  },
  {
    id: 'mistral',
    label: 'Niki + Mistral',
    command: 'export MISTRAL_API_KEY=... && niki',
  },
  {
    id: 'xai',
    label: 'Niki + xAI',
    command: 'export XAI_API_KEY=... && niki',
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
  { id: 'mistral', name: 'Mistral' },
  { id: 'xai', name: 'xAI' },
];

export const MEGA_MENU_CAPABILITIES = [
  {
    id: 'independence',
    icon: 'layers' as const,
    title: 'One Binary',
    desc: 'No runtime, package manager, or virtual environment.',
    detailId: 'independence',
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
    desc: 'Four themes, floors asserted by cargo test.',
    detailId: 'themes-system',
  },
  {
    id: 'sessions',
    icon: 'motion' as const,
    title: 'Session Resume',
    desc: 'SQLite sessions, /threads, /resume and /compact.',
    detailId: 'sessions',
  },
  {
    id: 'working-tree',
    icon: 'diff' as const,
    title: 'Working-Tree Diff',
    desc: '/diff renders staged, unstaged, and untracked changes.',
    detailId: 'working-tree',
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
    desc: 'One binary, session store, themes & approvals.',
    href: '#parallax-story',
  },
  {
    level: '03',
    title: '6-Stage Orbital Dial',
    desc: 'Interactive Boot → Binary → Theme → Keys → Sessions → Route view.',
    href: '#lifecycle',
  },
  {
    level: '04',
    title: 'Verified Baselines',
    desc: 'Install, sessions, contrast, and keymap checks.',
    href: '#benchmarks',
  },
];

export const MEGA_MENU_GUIDES = [
  {
    id: 'guide-install',
    badge: 'INSTALL',
    title: 'Install Niki Agent in one line',
    terminalCmd: '$ curl -fsSL …/install/install.sh | bash',
    terminalOut: '✓ SHA256SUMS verified · niki installed',
    detailId: 'install',
  },
  {
    id: 'guide-toml',
    badge: 'ONE TOML BLOCK',
    title: 'Add any OpenAI-compatible endpoint',
    terminalCmd: '[providers.gateway]',
    terminalOut: 'protocol = "openai"',
    detailId: 'providers',
  },
];

export const MEGA_MENU_BASELINES = [
  {
    id: 'one-binary',
    metric: '1',
    title: 'One Binary',
    desc: 'No runtime, package manager, or virtual environment.',
    detailId: 'binary',
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
    { label: 'One Binary', detailId: 'binary' },
    { label: 'Runtime Themes (/theme)', detailId: 'themes-system' },
    { label: 'Live Composer', detailId: 'adaptive-motion' },
    { label: 'Working-Tree Diff (/diff)', detailId: 'working-tree' },
    { label: 'Session Store', detailId: 'sessions' },
    { label: 'Live Keymap Registry', detailId: 'keymap' },
    { label: 'Providers', detailId: 'providers' },
    { label: 'Test Suite', detailId: 'ci-tests' },
  ],
  docs: [
    { label: 'Documentation site', href: 'https://niki-docs.vercel.app' },
    { label: 'Changelog', href: 'https://niki-docs.vercel.app/changelog' },
    { label: 'Beta builds', href: 'https://niki-docs.vercel.app/beta' },
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
    { label: 'Rust', href: 'https://www.rust-lang.org' },
    { label: 'ratatui', href: 'https://ratatui.rs' },
    { label: 'SQLite', href: 'https://sqlite.org' },
    { label: 'MIT NOTICE', href: 'https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/NOTICE' },
  ],
};
