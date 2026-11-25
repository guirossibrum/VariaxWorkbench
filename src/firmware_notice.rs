//! Display-only assessment of identities already read by an approved operation.

use crate::identity_stage::{BRIDGE_VERSION, GUITAR_VERSION};
use line6_devices::{
    family::{podxt, variax},
    identity::Identity,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Device {
    Podxt,
    Variax,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirmwareNotice {
    device: Device,
    reported: Option<(u8, u8)>,
    wrong_model: bool,
}

impl FirmwareNotice {
    pub fn variax(identity: &Identity) -> Option<Self> {
        use variax::firmware_read::FirmwareStatus;

        let status = if variax::procedures::check_identity(identity).is_ok() {
            FirmwareStatus::Verified310
        } else {
            FirmwareStatus::Unverified
        };
        (status == FirmwareStatus::Unverified).then(|| {
            Self::new(
                Device::Variax,
                identity,
                identity.is_line6()
                    && identity.family == variax::IDENTITY_FAMILY
                    && identity.member == 4,
            )
        })
    }

    pub fn podxt(identity: &Identity) -> Option<Self> {
        let model_matches = identity.is_line6()
            && identity.family == podxt::IDENTITY_FAMILY
            && identity.member == podxt::PODXT_LIVE_MEMBER;
        (!model_matches || identity.version != [0, 3, 0, 1])
            .then(|| Self::new(Device::Podxt, identity, model_matches))
    }

    fn new(device: Device, identity: &Identity, model_matches: bool) -> Self {
        let revision = identity.version;
        let digits = if revision.iter().all(u8::is_ascii_digit) {
            Some(revision.map(|digit| digit - b'0'))
        } else if revision.iter().all(|&digit| digit <= 9) {
            Some(revision)
        } else {
            None
        };
        Self {
            device,
            reported: digits.map(|[a, b, c, d]| (a * 10 + b, c * 10 + d)),
            wrong_model: !model_matches,
        }
    }

    pub fn device(self) -> Device {
        self.device
    }

    pub fn message(self) -> String {
        let (model, verified) = match self.device {
            Device::Podxt => ("PODxt Live", BRIDGE_VERSION),
            Device::Variax => ("Variax 300/600", GUITAR_VERSION),
        };
        let name = if self.wrong_model {
            "The connected device"
        } else {
            model
        };
        let mut text = match self.reported {
            Some((major, minor)) => format!("{name} reports firmware {major}.{minor:02}."),
            None => format!("{name} did not report a readable firmware version."),
        };
        if self.wrong_model {
            text.push_str(" Its model could not be verified.");
        }
        text.push_str(&format!(
            " Variax Workbench has only been verified with {model} firmware {verified}. You can open, edit and save files on this computer. Device actions are unavailable for this connection. Please run Rig Check and share its report."
        ));
        text
    }
}
