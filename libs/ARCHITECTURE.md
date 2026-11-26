# Agent SDK architecture

This document explains how the agent SDK inside `libs/code/niki_agent/sdk/` is
put together and where to look when a behavior needs to change. It is for new
maintainers and contributors.

For setup and day-to-day commands, see [`DEVELOPMENT.md`](./DEVELOPMENT.md).
For the product-level map — the TUI, the engine process, and the request
lifecycle — see [`docs/niki/ARCHITECTURE.md`](../docs/niki/ARCHITECTURE.md).

## The three layers

The SDK is a harness on top of a graph runtime and an agent abstraction. Most
questions become easier once you know which layer owns the behavior you are
looking at.

```txt
harness      opinionated defaults, middleware, backends, profiles
agent loop   model + tools + middleware -> one turn
runtime      state, checkpoints, streaming, interrupts
```

- **Runtime.** Runs the agent as a graph: steps connected by transitions, each
  able to read and update shared state. It carries state between steps, streams
  events from a live run, saves checkpoints, and pauses or resumes through
  interrupts.
- **Agent loop.** The caller describes an agent as a model, a set of tools and a
  middleware list; the loop calls the model, runs the tools it asks for, and
  repeats until the model stops asking.
- **Harness.** Assembles the default middleware stack and configures backends,
  subagents, skills, memory and profiles. It adds no new runtime; it packages
  what long-running agents need to be useful out of the box.

Harness code lives in three places: **middleware** adds behavior to the loop,
**backends** decide where files, memory and shell execution live, and
**profiles** tune the harness for a provider or model family.

## Construction and execution

There are two phases: **construction**, when the application asks for an agent,
and **execution**, when the returned graph runs.

### Construction

`graph.py` is the assembly point. It resolves the chat model and any profile
that applies to it, resolves the backend used by filesystem, skills, memory and
`execute` behavior, assembles the main-agent middleware stack, builds the
default general-purpose subagent plus any caller-provided ones, composes the
system prompt from caller instructions and SDK defaults, and produces the
runnable graph.

### Execution

On each turn the runtime drives the loop. The model receives a message history,
a system prompt and a set of tools. It may answer directly or request tool
calls; results are appended to state and the loop continues until it produces a
final response.

The harness changes that loop mainly through middleware. Middleware runs before
a model call, around it, around tool execution, or while state is prepared,
which lets it do things a plain tool cannot:

- add or drop tools from the current request,
- inject filesystem, memory, skills, subagent or approval instructions into the
  system prompt,
- summarize, compact or offload history as context grows,
- store typed values in graph state for later middleware or tools,
- enforce filesystem policy before a built-in filesystem tool runs.

A callable passed as a tool is different: it runs only after the model chooses
it, and cannot rewrite the tool list or the prompt. `middleware/__init__.py`
spells out the distinction.

## Middleware stack

Scaffolding middleware is installed first, caller middleware goes in the middle,
and tail middleware — which depends on the final prompt and tool surface — is
appended last:

- **Base scaffolding** creates what an agent needs to work: planning,
  filesystem access, subagent delegation, summarization, request cleanup.
- **Caller middleware** is where the application adds its own behavior without
  rebuilding the harness.
- **Profile and tail middleware** tune the final request: provider specifics,
  tool exclusions, prompt caching, memory injection, human approval.

Subagents have their own stacks, so a behavior can come from the main stack, a
declarative subagent stack, a compiled subagent, or a remote one. When a
behavior only appears during delegated work, find out which subagent type ran
before touching main-agent middleware.

## Tool surface and filesystem access

The model's visible tools are the result of several layers:

- built-in middleware contributes the standard tools — todo management,
  filesystem access, subagent delegation;
- caller-provided tools are added to that set;
- the resolved backend decides whether shell execution exists; without it the
  `execute` tool is removed and its prompt text is omitted;
- profiles can hide tools with `excluded_tools` for a model that needs a
  smaller or differently described surface;
- filesystem permissions enforce path policy on the built-in tools. They are
  not a visibility mechanism: a visible tool can still be denied at call time.

If a tool is missing, look at middleware assembly and profile exclusions. If it
is visible but fails, look at backend capability and permission enforcement.

## State and persistence

The runtime owns state. The SDK extends the base agent state with a message
field that uses a delta reducer, so checkpoint growth stays linear across long
threads. Middleware can contribute typed state fields through `state_schema`;
private fields are tracked so internal state does not leak into subagents. Use
middleware-owned state when the value only matters to that middleware, and a
graph-level field when tools or several middleware need to share it.

Persistence has two separate pieces:

- **Graph state and checkpoints** preserve conversation state, message history,
  interrupts and resumability.
- **Filesystem and memory persistence** comes through backends. The default
  state backend is thread-scoped; store-backed or filesystem-backed routes make
  files durable across threads or map them to disk or sandbox storage.

## Common starting points

Paths are relative to `libs/code/niki_agent/sdk/`.

- Construction, middleware ordering, prompt assembly, default model behavior:
  `graph.py`.
- Tool visibility, prompt injection, request-time behavior: `middleware/`.
  Feature modules include `subagents.py`, `filesystem.py`, `skills.py`,
  `memory.py`, `permissions.py` and `summarization.py`.
- Filesystem persistence, shell support, routing: `backends/`.
- Provider- or model-specific harness changes: `profiles/`.
- Public imports and compatibility expectations: `__init__.py`.
- The vendored SDK's upstream commit record: `PROVENANCE.md`.

When in doubt, trace from the public argument in `graph.py` to the middleware or
backend it installs, then follow how that component participates at run time.
