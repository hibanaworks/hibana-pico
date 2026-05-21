# UNO Q Heterogeneous Face Proof Plan

This example proves a deliberately small three-role path:

```text
WASI P1 guest process on Linux
  -> ChoreoFS /llm/stdout write
  -> local LLM sees the WASI shell terminal
  -> ChoreoFS /llm/stdin read
  -> local LLM sends shell commands
  -> ChoreoFS /face/frame write
  -> physical M33 LED face update
```

The authority source is the `hibana::g` choreography in `src/lib.rs`.
The WASI guest never calls a board API and never mutates shared host state. It
is the shell/terminal visible to the local LLM: it prints a prompt, accepts
`ls`, prints the ChoreoFS catalog, accepts `echo <code> > /face/frame`, and
only then writes the resulting `FaceFrame` bytes to `/face/frame` through WASI
imports admitted by the projected Endpoint. The guest is an ordinary WASI std
program with `fn main`; it does not use a custom `no_std` / `no_main` entry.

## LLM Confinement Model

The LLM is intentionally attached to a terminal-like WASI guest shell, not to the
M33 role, not to a transport handle, and not to a typed `FaceFrame` API. The
only world it can observe is the shell transcript exposed through `/llm/stdout`;
the only input it can provide is one shell command returned through
`/llm/stdin`.

Discovery is also part of the confinement model. The first command is `ls`,
which lets the LLM see the ChoreoFS catalog. That catalog is the capability view:
it tells the LLM which virtual objects exist and which operations are admitted.
For this proof the relevant advertised capability is:

```text
w /face/frame FaceFrame
```

Here `w` means the shell may write a `FaceFrame` payload to `/face/frame`.

After discovery, the LLM may request a face update only by returning:

```text
echo <code> > /face/frame
```

The WASI guest parses that shell command and performs the `/face/frame` write
only if the projected choreography is at the matching import step. The LLM text
does not become route authority, does not bypass WASI, and does not communicate
with M33 directly. This is the intended safety design: LLM capability is bounded
by the ChoreoFS view that the WASI guest exposes, and hibana's choreography
remains the authority for which effects can happen.

## Roles

- `ROLE_M33_LED_KERNEL = 0`: physical face renderer and `/face/frame`
  ChoreoFS driver.
- `ROLE_WASI_LLM_CELL = 1`: WASI P1 guest engine.
- `ROLE_LOCAL_LLM = 2`: local LLM boundary that services `/llm/stdout` and
  `/llm/stdin`. On Linux it can invoke an external LLM command with the WASI
  shell transcript and returns exactly one shell command. llama.cpp's upstream
  `llama-cli` is one acceptable command, but this proof does not depend on an
  Arduino-specific fork or repository. Without local LLM configuration it uses
  the small scripted shell-command source required by CI and hardware smoke
  tests.

No iOS ingress, Challenger network role, or detached unused sidecar choreography
is part of this proof.

## Choreography

1. WASI opens `/llm/stdin` for read.
2. WASI opens `/llm/stdout` for write.
3. WASI opens `/face/frame` for write.
4. WASI enters the projected import route loop.
5. Continue arm:
   - WASI writes the shell prompt to `/llm/stdout`.
   - The local LLM reads the terminal transcript and replies on `/llm/stdin`
     with `ls`.
   - WASI writes the ChoreoFS catalog to `/llm/stdout`.
   - The local LLM replies with `echo <code> > /face/frame`.
   - WASI parses that shell command into `FaceFrame` bytes and writes them to
     `/face/frame`.
   - M33 decodes the write as `FaceFrame`, validates fd/object/ordinal, and
     updates the LED matrix.
   - WASI immediately returns to the next shell prompt.
6. Break arm:
   - Bounded proof guests send `proc_exit(0)`.
   - The projected Endpoint admits that exit as terminal messages to both the
     local LLM role and the M33 role, so neither passive role guesses that the
     loop ended.
   - The real face demo keeps selecting the continue arm forever.

`FaceFrame` remains one typed message payload. Individual face patterns are
payload values, not separate message types and not route authority. LLM text is
not route authority either; it is terminal input consumed by the WASI shell.
M33 and the local LLM never exchange typed messages directly; WASI is the
isolation boundary.

## Cadence

- Emotion frames hold for 0.5 seconds.
- Mouth frames hold for 0.25 seconds.

The configured local LLM command receives the WASI shell transcript and should
return exactly one command:

```text
ls
echo h > /face/frame
```

The shell catalog advertises `w /face/frame FaceFrame`. The prompt documents
the compact face codes: `h`, `a`, `s`, `u`, `mc`, `ms`, `mw`, and `mr`.

The fallback scripted source returns commands for this cycle:

- 12 emotion frames cycling happy, angry, sad, surprised.
- 8 speaking mouth frames.

The bounded proof guest copies one cycle and exits. The real hardware face demo
uses `UNO_Q_FACE_LOOP_FOREVER=1`, swaps in the infinite shell-loop guest, and lets
the local LLM boundary drive the WASI shell forever.

## Local LLM Command Configuration

No local LLM implementation is vendored into this repository. The Linux side
calls an already installed command only when one of these variables is set:

- `UNO_Q_LOCAL_LLM_MODEL=/path/to/model.gguf`
- `UNO_Q_LOCAL_LLM_CLI=/path/to/llama-cli`
- `UNO_Q_LOCAL_LLM_CMD="custom command that prints face labels"`

When `UNO_Q_LOCAL_LLM_MODEL` is set without an explicit command, the default
constructed command targets an upstream llama.cpp-style CLI:

```text
llama-cli -m "$UNO_Q_LOCAL_LLM_MODEL" --no-display-prompt -n 64 --temp 0.2 -p "$UNO_Q_LOCAL_LLM_PROMPT"
```

`UNO_Q_LOCAL_LLM_ARGS` can replace the default generation flags, and
the shell transcript is supplied to the command on stdin. For the default
llama.cpp-style command, the transcript is also appended to the generated
prompt.

## Rendering

The normal/speaking eyes are two columns wide by three rows tall. The renderer
changes the face only when the projected Endpoint admits and decodes a
`/face/frame` write.

## Success Criteria

- `host-loopback-proof` passes with roles 0, 1, and 2.
- `uno-q-local-llm` is a separate process image for role 2.
- `uno-q-hardware-proof` passes with M33 as the physical peer and Linux running
  the WASI role plus the local LLM role.
- The bounded proof makes `HIBANA_M33_FACE_UPDATES` reach one cycle.
- The infinite face demo keeps increasing `HIBANA_M33_FACE_UPDATES` after the
  first cycle.
- No disconnected iOS, Challenger, or unused LLM-sidecar choreography is present.
