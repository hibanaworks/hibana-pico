# UNO Q Heterogeneous Face Proof Plan

This example proves a deliberately small four-role path:

```text
HumanInput CLI role on Linux
  -> typed HumanInputText message
  -> local LLM role on Linux
  -> one shell command
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
`ls` or `find ChoreoFS -type f`, prints the ChoreoFS catalog, accepts
`echo <code> > /face/frame`, and only then writes the resulting `FaceFrame`
bytes to `/face/frame` through WASI imports admitted by the projected Endpoint.
The guest is an ordinary WASI std program with `fn main`; it does not use a
custom `no_std` / `no_main` entry.

## LLM Confinement Model

The LLM is intentionally attached to a terminal-like WASI guest shell, not to the
M33 role, not to a transport handle, and not to a typed `FaceFrame` API. The
only world it can observe is the shell transcript exposed through `/llm/stdout`;
the only input it can provide is one shell command returned through
`/llm/stdin`.

Discovery is also part of the confinement model. The first command is `ls` or
the shell-equivalent `find ChoreoFS -type f`, which lets the LLM see the
ChoreoFS catalog. That catalog is the capability view: it tells the LLM which
virtual objects exist and which operations are admitted. For this proof the
relevant advertised capability is:

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
  `/llm/stdin`. On Linux it invokes an external LLM command with the WASI shell
  transcript and returns one terminal input line. The default Uno Q placement
  uses one persistent upstream llama.cpp `llama-server`; the Yzma/Project Hub
  path is only a reference that confirms llama.cpp runs on Uno Q Linux, not a
  dependency or fork. Scripted output is allowed only when explicitly requested
  with `UNO_Q_LOCAL_LLM_SCRIPTED=1` for host-only smoke checks.
- `ROLE_HUMAN_INPUT = 3`: HumanInput role. On Linux it is started by the CLI and
  can run either a prompt shell or a voice shell. It waits for the
  choreography-visible input request, then sends the input text as a typed
  `HumanInputText` message to the local LLM role; it does not classify,
  rewrite, or convert the text into face commands.

No iOS ingress, Challenger network role, or detached unused sidecar choreography
is part of this proof.

## Choreography

1. WASI opens `/llm/stdin` for read.
2. WASI opens `/llm/stdout` for write.
3. WASI opens `/face/frame` for write.
4. WASI writes the initial shell prompt to `/llm/stdout`.
5. WASI enters the projected import route loop.
6. Continue arm:
   - WASI asks the local LLM for the next terminal input line.
   - The local LLM sends a typed `HumanInputReq` to the HumanInput role.
   - The HumanInput role sends the latest input text to the local LLM role as
     one typed `HumanInputText` message.
   - The local LLM acknowledges that input turn with one typed
     `HumanInputAck` message.
   - The local LLM reads the terminal transcript and replies on `/llm/stdin`
     with `ls` or `find ChoreoFS -type f`.
   - WASI writes the ChoreoFS catalog to `/llm/stdout`.
   - WASI asks the local LLM for the next terminal input line.
   - The local LLM sends another typed `HumanInputReq` for this input turn.
   - The HumanInput role sends the latest input text to the local LLM role as
     one typed `HumanInputText` message.
   - The local LLM acknowledges that input turn with one typed
     `HumanInputAck` message, then answers the WASI read.
   - The local LLM replies with `echo <code> > /face/frame`.
   - WASI parses that shell command into `FaceFrame` bytes and writes them to
     `/face/frame`.
   - M33 decodes the write as `FaceFrame`, validates fd/object/ordinal, and
     updates the LED matrix.
   - WASI writes the next shell prompt to `/llm/stdout`.
7. Break arm:
   - Bounded proof guests send `proc_exit(0)`.
   - The projected Endpoint admits that exit as terminal messages to the local
     LLM role, the M33 role, and the HumanInput role, so no passive role guesses
     that the loop ended.
   - The real face demo keeps selecting the continue arm forever.

`FaceFrame` remains one typed message payload. Individual face patterns are
payload values, not separate message types and not route authority. LLM text is
not route authority either; it is terminal input consumed by the WASI shell.
M33 and the local LLM never exchange typed messages directly; WASI is the
isolation boundary. HumanInput also never talks to M33.

## Cadence

- Emotion frames hold for 0.5 seconds.
- Mouth frames hold for 0.25 seconds.

The configured local LLM command receives the WASI shell transcript and should
return exactly one command:

```text
ls
find ChoreoFS -type f
echo h > /face/frame
```

`ls` and `find ChoreoFS -type f` are equivalent discovery commands in this
small shell. The shell catalog advertises `w /face/frame FaceFrame`. The prompt
documents the compact face codes: `h`, `a`, `s`, `u`, `mc`, `ms`, `mw`, and
`mr`; the shell also accepts `v` as a surprised alias because the local model
occasionally emits it for that face.

The scripted source, when explicitly enabled for host-only checks, returns
commands for this cycle:

- 12 emotion frames cycling happy, angry, sad, surprised.
- 8 speaking mouth frames.

The bounded proof guest copies one cycle and exits. The real hardware face demo
uses `UNO_Q_FACE_LOOP_FOREVER=1`, swaps in the infinite shell-loop guest, and lets
the local LLM boundary drive the WASI shell forever.

## Local LLM Command Configuration

No local LLM implementation or model is vendored into this repository. The Uno Q
Linux side auto-detects the standard local placement made by
`scripts/install_llama_cpp_local_llm.sh`:

```text
/data/local/tmp/uno-q-local-llm/bin/llama-completion
/data/local/tmp/uno-q-local-llm/bin/llama-server
/data/local/tmp/uno-q-local-llm/models/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf
```

The model currently used for the proof is
`bartowski/Qwen2.5-0.5B-Instruct-GGUF/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf`,
which fits the Uno Q Linux memory budget and follows the few-shot terminal-input
prompt better than the smaller SmolLM2 models in this shell-command task. The
helper script also installs the llama.cpp shared libraries beside the binary
because upstream's backend loader expects them to be discoverable at process
start.

These variables override the default placement:

- `UNO_Q_LOCAL_LLM_MODEL=/path/to/model.gguf`
- `UNO_Q_LOCAL_LLM_SERVER=/path/to/llama-server`
- `UNO_Q_LOCAL_LLM_SERVER_ENDPOINT=http://127.0.0.1:18080`
- `UNO_Q_LOCAL_LLM_SERVER_PORT=18080`
- `UNO_Q_LOCAL_LLM_SERVER_ARGS="extra llama-server args"`
- `UNO_Q_LOCAL_LLM_CLI=/path/to/llama-completion`
- `UNO_Q_LOCAL_LLM_CMD="custom command that prints face labels"`
- `UNO_Q_LOCAL_LLM_LD_LIBRARY_PATH=/path/to/libs`
- `UNO_Q_LOCAL_LLM_WORK_DIR=/path/to/bin`
- `UNO_Q_HUMAN_INPUT_MODE=prompt` to run the prompt shell input role
- `UNO_Q_HUMAN_INPUT_MODE=voice` to run the voice shell input role
- `UNO_Q_HUMAN_INPUT_TEXT="initial human request text"`
- `UNO_Q_HUMAN_INPUT_VOICE_CMD="command that prints recognized utterances"`
- `UNO_Q_LOCAL_LLM_SELF_MOOD=1`
- `UNO_Q_LOCAL_LLM_SELF_MOOD_PROMPT="assistant mood instruction"`
- `UNO_Q_LOCAL_LLM_SCRIPTED=1` for host-only scripted smoke checks

When a model is configured without an explicit command, the default path starts
one persistent upstream llama.cpp server, waits for `/health`, and reuses that
loaded model for every terminal-input turn:

```text
llama-server -m "$UNO_Q_LOCAL_LLM_MODEL" --host 127.0.0.1 --port 18080 -t 4 -c 512 -np 1 --no-warmup --no-webui --no-slots --temp 0 -n 8
POST http://127.0.0.1:18080/completion
```

`llama-completion` remains only as an explicit fallback through
`UNO_Q_LOCAL_LLM_CLI` or custom experiments. The proof/demo path must not reload
the model per shell command.

No llama.cpp grammar is installed by default. The local LLM boundary copies the
first non-empty generated line as terminal input for the WASI guest. That means
the LLM may type any shell-looking text; invalid commands are rejected by the
WASI shell / ChoreoFS path, and actual effects are still admitted only by the
projected choreography. The shell transcript is included in the completion
prompt sent to the persistent server. `UNO_Q_LOCAL_LLM_ARGS` can replace the
fallback completion flags for manual experiments.

The hardware CLI can start the live input role directly:

```text
uno-q-hardware-proof --prompt-shell
uno-q-hardware-proof --voice-shell --voice-cmd "speech-to-text-command"
```

The prompt shell reads terminal lines. The voice shell starts
`UNO_Q_HUMAN_INPUT_VOICE_CMD` and reads recognized utterances from that process'
stdout. In both modes the input role strips only the terminal line delimiter as
transport framing, validates the fixed-capacity UTF-8 typed payload, and sends
the remaining text unchanged to the local LLM role. The input role does not
classify, rewrite, or convert the text into face commands. The local LLM receives
the exact text as prompt context and remains the only component that chooses the
next shell command.

The old prompt-file injection path is not
part of the demo: human input is a live terminal interaction, not a file that a
sidecar rewrites. No model restart, proof restart, or choreography change is
required for the next turn to observe the new human input.

Human text is prompt context for the LLM only. It never becomes route authority,
never bypasses WASI, and never writes `/face/frame` directly. The input role does
not classify, rewrite, or convert the text; the local LLM decides what shell
command to emit, and the shell parser plus projected ChoreoFS write are the
enforcement points.

The face-choice LLM prompt uses few-shot terminal examples rather than a hard
grammar: happy maps to `echo h > /face/frame`, frustrated maps to
`echo a > /face/frame`, sad maps to `echo s > /face/frame`, and surprised maps
to `echo u > /face/frame`. This is only prompt guidance; the executable
authority is still the WASI shell parser plus hibana choreography.

For experiments, `UNO_Q_LOCAL_LLM_SELF_MOOD=1` lets the local LLM choose from the
same finite face command set based on an assistant-mood instruction instead of a
human request. This is intentionally an opt-in demo mode: without it and without
a human request, the proof keeps the deterministic face cycle. The default
self-mood instruction cycles simulated moods across turns, and the prompt asks
the model to complete the matching terminal command from few-shot examples.
This is still prompt shaping only; it is not a llama.cpp grammar, typed API, or
side channel.

## Rendering

The normal/speaking eyes are two columns wide by three rows tall. The renderer
changes the face only when the projected Endpoint admits and decodes a
`/face/frame` write.

## Success Criteria

- `host-loopback-proof` passes with roles 0, 1, 2, and 3.
- `uno-q-hardware-proof` passes with M33 as the physical peer and Linux running
  the WASI role plus the local LLM role plus the HumanInput role.
- The bounded proof makes `HIBANA_M33_FACE_UPDATES` reach one cycle.
- The infinite face demo keeps increasing `HIBANA_M33_FACE_UPDATES` after the
  first cycle.
- No disconnected iOS, Challenger, or unused LLM-sidecar choreography is present.
