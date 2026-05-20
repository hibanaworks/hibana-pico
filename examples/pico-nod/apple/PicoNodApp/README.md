# Pico Nod App

This is the minimal Apple app surface for Pico Nod approval.

It is intentionally small:

```text
display exact approval request
compute displayed hash
sign Nod / Reject / Fence evidence
show signed evidence
```

It must not:

```text
select Hibana routes
call external action APIs
hold external action credentials
hold APNs provider credentials
commit side effects
```

Current verification:

```bash
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test
scripts/check_pico_nod_app.sh
```

Release readiness check:

```bash
scripts/check_pico_nod_release_readiness.sh
```

This check is expected to fail until production Apple and boundary identifiers
are configured.

The matching server preflight is:

```bash
cargo run -p pico-nod-example --bin pico-nod-http-acceptor -- --preflight
```

Production HTTP is loopback-only. Public TLS must terminate before forwarding to
the server.

The App Store archive/export path is:

```bash
scripts/archive_pico_nod_app.sh
```

It uses the checked-in `PicoNodApp.xcodeproj` and shared `PicoNodApp` scheme.

App Store release still requires:

```text
Apple Developer Program team
Bundle ID
provisioning profile
Xcode first launch setup
production APNs entitlement
production APNs provider key material
App Store Server key material
external action credential material
Archive/export through Xcode
privacy labels and review metadata
```

The checked-in Xcode project includes the minimal app icon asset catalog,
privacy manifest, entitlements, launch screen, and shared archive scheme. The
app gate builds the iOS Release target without signing and fails on actionable
Xcode warnings. The known AppIntents metadata skip emitted for apps that do not
use AppIntents is ignored; adding AppIntents only to silence that line would
increase the app surface without value.

The repository keeps the release audit notes here:

```text
examples/pico-nod/release/app-store-review.md
examples/pico-nod/release/privacy-labels.md
examples/pico-nod/release/operations-runbook.md
```
