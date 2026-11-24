use crate::editor::{detection::DeviceKey, tabs::DeviceTab};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Attachment {
    pub(super) key: DeviceKey,
    pub(super) path: String,
    pub(super) address: u8,
}

impl Attachment {
    pub(super) fn verified(device: &DeviceTab) -> Option<Self> {
        let (path, address) = device.verified_attachment()?;
        Some(Self {
            key: device.device.key.clone(),
            path: path.into(),
            address,
        })
    }
}
