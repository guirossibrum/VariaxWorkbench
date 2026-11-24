//! Endpoints for the first-generation Variax control link.
use crate::session::Path;
use crate::transport::InterfaceSpec;
use std::time::Duration;

impl Path {
    // Keep the existing request and tool path name (#214).
    #[allow(non_upper_case_globals)]
    pub const VariaxViaPodxt: Self = Self::new("VariaxViaPodxt");
}

/// Variax control through PODxt Live USB interface 1 (kernel `snd_usb_variax`).
pub const VARIAX_VIA_PODXT_LIVE: InterfaceSpec = InterfaceSpec {
    vendor: 0x0e41,
    product: 0x4650,
    interface: 1,
    altsetting: 1,
    endpoint_out: 0x05,
    endpoint_in: 0x86,
    packet: 16,
    chunk_gap: Duration::from_millis(5),
    path: Path::VariaxViaPodxt,
};
