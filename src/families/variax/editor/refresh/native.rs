//! Bind a verified editor attachment to its native Variax library route.

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
use super::DownloadRoute;
use super::{Attachment, RouteCapability};
#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
use line6_devices::{device_lock::DeviceLock, transport::usb::UsbInterfaceLink};

#[cfg(any(test, target_os = "linux"))]
pub(super) fn capability(_: &Attachment) -> RouteCapability {
    Ok(DownloadRoute {
        #[cfg(test)]
        download_on_connect: true,
    })
}

#[cfg(any(test, not(any(target_os = "linux", target_os = "macos"))))]
pub(super) fn unsupported_capability(_: &Attachment) -> RouteCapability {
    Err("Guitar download requires a native USB adapter. Open a saved library for offline editing.")
}

#[cfg(all(not(test), not(any(target_os = "linux", target_os = "macos"))))]
pub(super) use unsupported_capability as capability;

#[cfg(any(test, target_os = "macos"))]
pub(super) fn macos_capability(attachment: &Attachment) -> RouteCapability {
    macos_registry_id(attachment)?;
    // The startup window asks for consent before starting the sweep.
    Ok(DownloadRoute {
        #[cfg(test)]
        download_on_connect: false,
    })
}

#[cfg(all(not(test), target_os = "macos"))]
pub(super) use macos_capability as capability;

#[cfg(any(test, target_os = "macos"))]
fn macos_registry_id(attachment: &Attachment) -> Result<u64, &'static str> {
    attachment.key.attachment.usb_registry_id().ok_or(
        "Download needs a verified macOS USB attachment. Reconnect the PODxt Live and check the guitar again.",
    )
}

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
pub(super) fn open<'rig>(
    rig: &'rig DeviceLock,
    attachment: &Attachment,
) -> Result<UsbInterfaceLink<&'rig DeviceLock>, String> {
    use variax_workbench::usb;

    let coordinates = (attachment.path.as_str(), attachment.address);
    #[cfg(target_os = "linux")]
    return usb::open_variax_bridge_borrowed(rig, coordinates);
    #[cfg(target_os = "macos")]
    usb::open_macos_variax_bridge_borrowed(rig, macos_registry_id(attachment)?, coordinates)
}
