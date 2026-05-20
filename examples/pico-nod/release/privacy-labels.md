# Pico Nod Privacy Labels

This file is the source checklist for App Store privacy labels. The shipped app
surface is intentionally small and does not include analytics, advertising SDKs,
tracking, or direct external action credentials.

## App Data Handling

```text
No tracking.
No third-party advertising.
No analytics SDK.
No external action API credentials in the app.
No APNs provider private key in the app.
No StoreKit server private key in the app.
```

The app may receive notification payloads that contain bounded approval request
evidence. That evidence is displayed to the user and used to produce a local
decision signature. The app must not treat notification delivery as approval.

## Server Data Handling

Server-side data is choreography evidence:

```text
approval request hash
displayed hash
decision evidence
transaction id
receipt/export evidence
fault evidence
```

Pico Nod has no database in the core design. Any issuer or deployment that
keeps durable operational records must document that storage outside the app
privacy label source and must not turn storage into route authority.

## Redaction

Audit output must not contain:

```text
APNs provider private key material
StoreKit private key material
external action credentials
raw bearer tokens
raw device tokens
```
