# Pico Nod Operations Runbook

This runbook is required release evidence. Unit tests and local app builds are
not production readiness.

## Required Configuration

```text
PICO_NOD_APPLE_TEAM_ID
PICO_NOD_BUNDLE_ID
PICO_NOD_APNS_KEY_ID
PICO_NOD_APNS_TEAM_ID
PICO_NOD_APNS_TOPIC
PICO_NOD_APNS_PRIVATE_KEY_PATH
PICO_NOD_STORE_ISSUER_ID
PICO_NOD_STORE_KEY_ID
PICO_NOD_STORE_PRIVATE_KEY_PATH
PICO_NOD_TLS_TERMINATION=external-loopback
PICO_NOD_EXTERNAL_ACTION_ENDPOINT
PICO_NOD_EXTERNAL_ACTION_CREDENTIAL_PATH
```

Credential path variables must point to readable files owned by the deployment
operator. They are never stored in the repository.

## Production Shape

```text
public TLS
  -> external TLS terminator
  -> loopback pico-nod-http-acceptor
  -> WASI P1 ingress normalization
  -> Hibana choreography
  -> boundary-specific side effect
```

The acceptor must not bind public clear HTTP in production mode.

## Startup

1. Install the release app through App Store/TestFlight or a signed internal
   archive.
2. Provision APNs and App Store Server API keys.
3. Configure the launchd environment file from `deploy/env.example`.
4. Run `scripts/check_pico_nod_release_readiness.sh`.
5. Start the service through the checked-in launchd plist.

## Incident Handling

If APNs, StoreKit, or an external action boundary returns
`UnknownWithoutIdempotencyEvidence`, the session fences closed. Retry may only
happen through idempotency evidence for the same transaction id and body hash.

If credentials are suspected compromised:

```text
rotate provider key
rotate external action credential
invalidate affected issuer evidence
export receipts and fault evidence
restart service
```

No handler may infer approval from delivery, billing, logs, storage, or retry
state. Choreography remains the authority.
