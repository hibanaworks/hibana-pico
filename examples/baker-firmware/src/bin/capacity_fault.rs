#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

use baker_firmware::{BakerCapsuleFacts, BakerPlacement};
use hibana::g;
use hibana::runtime::wire::{CodecError, Payload, WireEncode, WirePayload};
use hibana_pico::appkit;

const LABEL_CAPACITY_PAYLOAD: u8 = 61;
const CAPACITY_PAYLOAD_BYTES: usize = 160;
const RESULT_CAPACITY_FAULT_OK: u32 = 0x4849_4341;

struct CapacityFault;
struct CapacityFaultLocal;

#[derive(Clone, Copy)]
struct CapacityPayload([u8; CAPACITY_PAYLOAD_BYTES]);

impl WireEncode for CapacityPayload {
    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < self.0.len() {
            return Err(CodecError::Truncated);
        }
        out[..self.0.len()].copy_from_slice(&self.0);
        Ok(self.0.len())
    }
}

impl WirePayload for CapacityPayload {
    type Decoded<'a> = Payload<'a>;

    fn validate_payload(input: Payload<'_>) -> Result<(), CodecError> {
        if input.as_bytes().len() == CAPACITY_PAYLOAD_BYTES {
            Ok(())
        } else {
            Err(CodecError::Malformed)
        }
    }

    fn decode_validated_payload<'a>(input: Payload<'a>) -> Self::Decoded<'a> {
        input
    }
}

impl appkit::Capsule for CapacityFault {
    type Placement = BakerPlacement;
    type Localside = CapacityFaultLocal;

    fn choreography() -> impl hibana::runtime::program::Projectable {
        g::send::<1, 0, g::Msg<LABEL_CAPACITY_PAYLOAD, CapacityPayload>>()
    }

    fn observe(tap: &mut hibana::runtime::tap::TapPort<'_>) {
        baker_firmware::poll_epf_diagnostic(tap);
    }
}

impl BakerCapsuleFacts for CapacityFault {
    const SUCCESS_RESULT: u32 = RESULT_CAPACITY_FAULT_OK;

    fn run_engine_image() {
        baker_firmware::run_engine_no_wasi::<Self>();
    }
}

impl appkit::Localside<CapacityFault> for CapacityFaultLocal {
    type Error = core::convert::Infallible;

    fn engine<'endpoint, const ROLE: u8>(
        mut ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        async move {
            if ROLE == 1 {
                let payload = CapacityPayload([0xa5; CAPACITY_PAYLOAD_BYTES]);
                let result = ctx
                    .send::<g::Msg<LABEL_CAPACITY_PAYLOAD, CapacityPayload>>(&payload)
                    .await;
                core::hint::black_box(&result);
            }
            appkit::pending(ctx).await
        }
    }

    fn driver<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        appkit::pending(ctx)
    }

    fn boundary<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        appkit::pending(ctx)
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    baker_firmware::panic(info)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn baker_selected_run() -> ! {
    baker_firmware::run::<CapacityFault>()
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    panic!("baker-firmware examples are RP2040 hardware artifacts; build for thumbv6m-none-eabi")
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    baker_firmware::run::<CapacityFault>()
}
