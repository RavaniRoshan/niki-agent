# Upstream Diff

Records edits to upstream-owned files made while evolving Niki.

## Task 4

- `niki_agent.adapters.runtime.acp.ACPAgentRuntime` now executes the
  checkpointed graph, emits assistant text and approval events, resumes with a
  LangGraph `Command`, and cancels its owned stream task. It does not import or
  depend on `AgentSessionContext`; that type configures the ACP server's graph
  factory and does not identify a live ACP session.
- Niki headless turns now pass through `RunTurnCoordinator` and
  `LangGraphRemoteRuntime`. The adapter forwards native chunks to the existing
  headless renderer, preserving its output and hook behavior. The legacy
  `dcode` path remains on its existing stream path.
- Niki TUI turns now pass through the same managed runtime. A bounded queue
  returns native chunks to the existing Textual processing loop while the
  coordinator validates lifecycle and resume state. Other entry points retain
  direct streaming.
- Niki ACP prompt graphs now pass through `RunTurnCoordinator` inside the
  existing `_AutoGraph` adapter. Native LangGraph chunks still feed the current
  ACP prompt handler, which retains ownership of session IDs, checkpoints,
  permission requests, replay, and protocol rendering. The same runtime turn ID
  and validated event sequence continue through ACP's internal approval resume.
- `dcode` and `deepagents-code` retain the direct ACP graph stream path because
  coordinator routing is enabled only inside `niki_runtime_scope()`.

## Task 5

- The server runtime now exposes a `WorkspacePort` adapter over the existing
  composite backend. Each workspace runtime binds that adapter to the validated
  workspace ID and root before use; file and command operations reject paths
  outside that root, including symlink escapes.
- The backend adapter uses `aread`, `awrite`, and `aexecute`, translates their
  result types, surfaces backend failures, and safely quotes shell arguments.
- `DefaultApprovalPolicy` now translates the live approval mode used by the
  existing `interrupt_on` predicate. It does not infer permissions from tool
  name prefixes or introduce a separate approval-mode store.
- The `ServerRuntime` tuple gained an optional trailing workspace port field;
  existing three-argument construction remains compatible.

## K1 / first-run focus

- `DeepAgentsApp.on_mount` no longer focuses the chat input when another screen
  is already on the stack. A first launch pushes `LaunchNameScreen` before the
  composer exists, so the synchronous focus call landed *behind* the modal: the
  name field showed a cursor, but keystrokes went to the hidden composer and
  "Enter to continue" did nothing. The guard leaves auto-focus to the screen on
  top, so returning users are unaffected. Verified by
  `test_first_run_name_prompt_owns_focus` and the pty onboarding flow.
- `NikiApp` overrides `_handle_command` for `/help` only, mounting the
  registry-generated body (`render_help_for`) instead of upstream's
  hand-maintained block. `dcode` keeps upstream's screen.

## Session 5 — composer surface

- `tui/widgets/chat_input.py` gained three additive pieces used by the Niki
  fork: an empty `ChatInput.COMPOSER_PLACEHOLDER`, a `#composer-queue` label with
  `set_queued()`, and a `ChatTextArea` binding for `ctrl+z` (suspend) that moves
  undo to `ctrl+shift+z`. The handler lives on the text area because widget
  bindings do not bubble to app-level actions. Both composer behaviors are
  opt-in per instance: `ChatTextArea.suspend_on_ctrl_z` defaults to `False` and
  `action_suspend_process` then routes the key to Textual's undo, so `dcode`
  keeps its Ctrl+Z and its hint-free composer. `NikiApp` sets the flag and the
  hint after mount, next to the existing cursor-blink preference.
- `niki/niki.tcss` moved the composer border from `#input-area` to
  `#input-area #input-box` so the mode accent and the incognito border title
  still apply; the wrapper keeps no border.
- `niki/app.py` now owns the `f2` mouse-release toggle, a tmux-aware
  `copy_to_clipboard`, a `suspend()` override that re-applies a released mouse,
  and a typed `push_screen` callback for the help modal. These are Niki-only
  overrides; `dcode` is untouched.
