# Pico Nod App Review Notes

Pico Nod is a local approval app for choreography-governed external intents.
It displays an exact intent summary, computes the displayed hash, and signs one
of three local decisions:

```text
Nod
Reject
Fence
```

The app does not select Hibana routes, call external action APIs, hold APNs
provider credentials, hold external action credentials, or commit side effects.

## Reviewer Flow

1. Launch Pico Nod.
2. Open the bundled sample intent screen.
3. Verify that the displayed hash changes when the text changes.
4. Tap Nod, Reject, or Fence.
5. Confirm that the signed evidence screen shows only the decision evidence.

No reviewer login is required for the local sample flow. Production APNs,
StoreKit, and external action credentials are server-side release facts and are
not embedded in the app.

## Review Boundary

```text
APNs notification
  -> displayed approval request
  -> local signed evidence
  -> server-side Hibana choreography
```

The app is an approval device, not a posting client and not an administration
console.
