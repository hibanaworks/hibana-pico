#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseRequirement {
    pub name: &'static str,
    pub purpose: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseArtifact {
    pub path: &'static str,
    pub purpose: &'static str,
}

pub const APPLE_TEAM_ID: &str = "PICO_NOD_APPLE_TEAM_ID";
pub const BUNDLE_ID: &str = "PICO_NOD_BUNDLE_ID";
pub const APNS_KEY_ID: &str = "PICO_NOD_APNS_KEY_ID";
pub const APNS_TEAM_ID: &str = "PICO_NOD_APNS_TEAM_ID";
pub const APNS_TOPIC: &str = "PICO_NOD_APNS_TOPIC";
pub const APNS_PRIVATE_KEY_PATH: &str = "PICO_NOD_APNS_PRIVATE_KEY_PATH";
pub const STORE_ISSUER_ID: &str = "PICO_NOD_STORE_ISSUER_ID";
pub const STORE_KEY_ID: &str = "PICO_NOD_STORE_KEY_ID";
pub const STORE_PRIVATE_KEY_PATH: &str = "PICO_NOD_STORE_PRIVATE_KEY_PATH";
pub const TLS_TERMINATION: &str = "PICO_NOD_TLS_TERMINATION";
pub const EXTERNAL_ACTION_ENDPOINT: &str = "PICO_NOD_EXTERNAL_ACTION_ENDPOINT";
pub const EXTERNAL_ACTION_CREDENTIAL_PATH: &str = "PICO_NOD_EXTERNAL_ACTION_CREDENTIAL_PATH";

pub const RELEASE_FILE_REQUIREMENTS: &[&str] = &[
    APNS_PRIVATE_KEY_PATH,
    STORE_PRIVATE_KEY_PATH,
    EXTERNAL_ACTION_CREDENTIAL_PATH,
];

pub const RELEASE_REQUIREMENTS: &[ReleaseRequirement] = &[
    ReleaseRequirement {
        name: APPLE_TEAM_ID,
        purpose: "Apple Developer Program team for signed app release",
    },
    ReleaseRequirement {
        name: BUNDLE_ID,
        purpose: "App Store bundle identity and entitlement profile",
    },
    ReleaseRequirement {
        name: APNS_KEY_ID,
        purpose: "APNs provider key identity",
    },
    ReleaseRequirement {
        name: APNS_TEAM_ID,
        purpose: "APNs provider team identity",
    },
    ReleaseRequirement {
        name: APNS_TOPIC,
        purpose: "APNs app topic",
    },
    ReleaseRequirement {
        name: APNS_PRIVATE_KEY_PATH,
        purpose: "APNs provider private key path",
    },
    ReleaseRequirement {
        name: STORE_ISSUER_ID,
        purpose: "App Store Server API issuer",
    },
    ReleaseRequirement {
        name: STORE_KEY_ID,
        purpose: "App Store Server API key",
    },
    ReleaseRequirement {
        name: STORE_PRIVATE_KEY_PATH,
        purpose: "App Store Server API private key path",
    },
    ReleaseRequirement {
        name: TLS_TERMINATION,
        purpose: "external-loopback TLS termination mode",
    },
    ReleaseRequirement {
        name: EXTERNAL_ACTION_ENDPOINT,
        purpose: "concrete external side-effect boundary endpoint",
    },
    ReleaseRequirement {
        name: EXTERNAL_ACTION_CREDENTIAL_PATH,
        purpose: "concrete external side-effect credential path",
    },
];

pub const RELEASE_ARTIFACTS: &[ReleaseArtifact] = &[
    ReleaseArtifact {
        path: "examples/pico-nod/release/app-store-review.md",
        purpose: "App Review metadata and reviewer verification path",
    },
    ReleaseArtifact {
        path: "examples/pico-nod/release/privacy-labels.md",
        purpose: "App Store privacy label source of truth",
    },
    ReleaseArtifact {
        path: "examples/pico-nod/release/operations-runbook.md",
        purpose: "production server operation and incident runbook",
    },
];
