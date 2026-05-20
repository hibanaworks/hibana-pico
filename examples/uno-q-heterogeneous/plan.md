# UNO Q Heterogeneous Face Proof Plan

This example proves a deliberately small three-role path:

```text
WASI P1 guest process on Linux
  -> ChoreoFS /llm/frame read
  -> pseudo LLM process through the WASI fd_read import
  -> WASI guest receives the face frame bytes
  -> ChoreoFS /face/frame write
  -> physical M33 LED face update
```

The authority source is the `hibana::g` choreography in `src/lib.rs`.
The WASI guest never calls a board API and never mutates shared face state.
It only performs WASI imports admitted by the projected Endpoint.

## Roles

- `ROLE_M33_LED_KERNEL = 0`: physical face renderer and `/face/frame`
  ChoreoFS driver.
- `ROLE_WASI_LLM_CELL = 1`: WASI P1 guest engine.
- `ROLE_PSEUDO_LLM = 2`: separate pseudo-LLM process that services
  `/llm/frame` through WASI `fd_read`.

No iOS ingress, Challenger network role, or detached unused sidecar choreography
is part of this proof.

## Choreography

1. WASI opens `/llm/frame` for read.
2. WASI opens `/face/frame` for write.
3. WASI enters the projected import route loop.
4. Continue arm:
   - WASI requests `fd_read(/llm/frame)` from the pseudo LLM role.
   - The pseudo LLM role replies to WASI with the exact two face-frame bytes.
   - WASI writes those exact bytes to `/face/frame`.
   - M33 decodes the write as `FaceFrame`, validates fd/object/ordinal, and
     updates the LED matrix.
   - WASI sleeps through `poll_oneoff` for the projected cadence.
5. Break arm:
   - WASI sends `proc_exit(0)`.

`FaceFrame` remains one typed message payload. Individual face patterns are
payload values, not separate message types and not route authority.
M33 and the pseudo LLM never exchange typed messages directly; WASI is the
isolation boundary.

## Cadence

- Emotion frames hold for 1 second.
- Mouth frames hold for 0.5 seconds.

The pseudo LLM returns 20 frames per proof cycle:

- 12 emotion frames cycling happy, angry, sad, surprised.
- 8 speaking mouth frames.

## Rendering

The normal/speaking eyes are two columns wide by three rows tall. The renderer
changes the face only when the projected Endpoint admits and decodes a
`/face/frame` write.

## Success Criteria

- `host-loopback-proof` passes with roles 0, 1, and 2.
- `uno-q-pseudo-llm` is a separate process image for role 2.
- `uno-q-hardware-proof` passes with M33 as the physical peer and Linux running
  the WASI role plus the pseudo LLM role.
- The M33 marker `HIBANA_M33_FACE_UPDATES` reaches the expected cycle count.
- No disconnected iOS, Challenger, or unused LLM-sidecar choreography is present.
