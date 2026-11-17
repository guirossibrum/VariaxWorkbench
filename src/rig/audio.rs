#![forbid(unsafe_code)]
//! Frozen source-row audio accounting and offline route admission.
//!
//! This registry is independent of settings codecs. An alias, inventory entry,
//! static driver declaration or successful plan never authorizes a stream.
use super::{ProductProfile, RoleKind};
use crate::pod_audio;
use sha2::{Digest, Sha256};
use std::io::{self, Write};

pub const SCHEMA: u8 = 2;
pub const ROW_COUNT: usize = 224;
/// Digest reported for the original schema-2 file, not recomputed from names.
pub const ORIGINAL_SOURCE_SHA256: &str =
    "e808fe477b4d1c8a976bdaf11369e711e131317164f20d4140b816f69dd26c7b";
/// Independently pinned public names/roles projection from issue 96.
pub const PROJECTION_SHA256: &str =
    "c3ed43ad76a0486ef327cb0cf1c42d389124c765418b0065b29f2d1db37ba790";
pub const SNAPSHOT: &str = include_str!("../../research/audio-inventory-schema2.tsv");
pub const CONTRACT: &str = "linux-v7.2.15-podxt-live-audio-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceRole {
    Alias,
    Product,
    Guitar,
    Bridge,
    Family,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Classification {
    /// A static declaration, not working live audio.
    NativeUsbDeclaration,
    /// Precautionary denial based on a maintenance-labelled source row.
    MaintenanceLabelDenied,
    Unresolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkGroup {
    PodXt,
    PodHd,
    HelixHx,
    PodGo,
    Amplifi,
    Firehawk,
    Catalyst,
    Spider,
    LegacyAmplifiers,
    LegacyPod,
    RecordingInterfaces,
    GuitarsAndBridges,
    Controllers,
    Wireless,
    EffectsAndRecorders,
}

/// Grouping is planning only: case/spacing never merges source rows or proves identity.
fn work_group(name: &str) -> WorkGroup {
    let n = name.to_ascii_lowercase();
    if n.starts_with("catalyst") {
        WorkGroup::Catalyst
    } else if n.contains("fbv") || n == "helix control" {
        WorkGroup::Controllers
    } else if n.starts_with("relay") || n.starts_with("xd-v") {
        WorkGroup::Wireless
    } else if n.contains("variax")
        || n.starts_with("jtv")
        || n.starts_with("shuriken")
        || n == "workbench hd"
    {
        WorkGroup::GuitarsAndBridges
    } else if n.contains("podxt") || n.contains("pod xt") {
        WorkGroup::PodXt
    } else if n.contains("pod hd") {
        WorkGroup::PodHd
    } else if n.starts_with("helix") || n.starts_with("hx ") {
        WorkGroup::HelixHx
    } else if n.starts_with("pod go") {
        WorkGroup::PodGo
    } else if n.starts_with("amplifi") || n == "fx100" {
        WorkGroup::Amplifi
    } else if n.starts_with("firehawk") {
        WorkGroup::Firehawk
    } else if n.starts_with("spider") {
        WorkGroup::Spider
    } else if n.contains("toneport")
        || n.contains("guitarport")
        || n == "guitar port"
        || n.starts_with("pod studio")
        || matches!(n.as_str(), "kb37" | "ux1" | "ux2" | "ux8" | "line 6 ux8")
    {
        WorkGroup::RecordingInterfaces
    } else if n.contains("pod") {
        WorkGroup::LegacyPod
    } else if n.starts_with("vetta")
        || n.starts_with("flextone")
        || n.starts_with("dt")
        || n.starts_with("hd147")
        || n == "hd 147"
        || n == "ax2"
        || n == "duoverb"
        || n.starts_with("powercab")
    {
        WorkGroup::LegacyAmplifiers
    } else {
        // Explicit bounded remainder: all names must match the reviewed list.
        WorkGroup::EffectsAndRecorders
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRow {
    /// One-based ordinal in the frozen projection; duplicate names stay distinct.
    pub ordinal: usize,
    pub name: String,
    pub role: SourceRole,
    pub classification: Classification,
    pub group: WorkGroup,
    pub gap: String,
}

#[derive(Debug)]
pub struct Registry {
    rows: Vec<SourceRow>,
}

impl Registry {
    /// Validate bytes, schema shape, order, roles and counts before exposing rows.
    pub fn from_snapshot(snapshot: &str) -> Result<Self, String> {
        Self::from_projection(snapshot, SCHEMA, ORIGINAL_SOURCE_SHA256)
    }

    /// Reject a changed original descriptor even if its names are unchanged.
    /// The original byte hash is a reported source pin, not reauthenticated here.
    pub fn from_projection(
        snapshot: &str,
        schema: u8,
        original_sha256: &str,
    ) -> Result<Self, String> {
        if schema != SCHEMA || original_sha256 != ORIGINAL_SOURCE_SHA256 {
            return Err(
                "schema or original source pin changed; reviewed migration required".into(),
            );
        }
        if format!("{:x}", Sha256::digest(snapshot.as_bytes())) != PROJECTION_SHA256 {
            return Err(
                "frozen names/roles projection drift; new reviewed version required".into(),
            );
        }
        let mut lines = snapshot.lines();
        if lines.next() != Some("name\trole") {
            return Err("invalid source header".into());
        }
        let mut rows = Vec::new();
        for (index, line) in lines.enumerate() {
            let (name, role) = line.split_once('\t').ok_or("invalid source row")?;
            let role = match role {
                "alias" => SourceRole::Alias,
                "product" => SourceRole::Product,
                "guitar" => SourceRole::Guitar,
                "bridge" => SourceRole::Bridge,
                "family" => SourceRole::Family,
                _ => return Err("unreviewed source role".into()),
            };
            let classification = if name == "PODxt Live" && role == SourceRole::Bridge {
                Classification::NativeUsbDeclaration
            } else if name.to_ascii_lowercase().contains("bootloader") {
                Classification::MaintenanceLabelDenied
            } else {
                Classification::Unresolved
            };
            let gap = match classification {
                Classification::NativeUsbDeclaration => "Static Linux declaration only; installed revision, OS negotiation, architecture builds, live backend and hardware validation unavailable".to_owned(),
                Classification::MaintenanceLabelDenied => format!("{name}: maintenance label denied; reviewed normal-runtime association and physical identity absent; never enter maintenance for recording"),
                Classification::Unresolved => format!("{name} row {}: {}", index + 1, match role {
                    SourceRole::Alias => "unreviewed alias/model/revision mapping; no endpoint or compatible external route contract admitted",
                    SourceRole::Family => "family label is not physical identity; variants/endpoints and per-OS capture evidence unresolved",
                    SourceRole::Guitar => "guitar variant/revision and compatible bridge or analog-interface signal contract unresolved; guitar identity is not USB audio",
                    _ => "recovered catalog identity is not audio evidence; exact endpoint, revision, native/external route and per-OS clock/format/driver contract unresolved",
                }),
            };
            rows.push(SourceRow {
                ordinal: index + 1,
                name: name.into(),
                role,
                classification,
                group: work_group(name),
                gap,
            });
        }
        if rows.len() != ROW_COUNT
            || rows.iter().filter(|r| r.role == SourceRole::Alias).count() != 186
        {
            return Err("source coverage counts differ".into());
        }
        Ok(Self { rows })
    }

    pub fn frozen() -> Result<Self, String> {
        Self::from_snapshot(SNAPSHOT)
    }

    pub fn rows(&self) -> &[SourceRow] {
        &self.rows
    }

    /// Machine-readable coverage; no user strings or runtime discovery enter it.
    pub fn write_tsv(&self, mut out: impl Write) -> io::Result<()> {
        writeln!(
            out,
            "#schema={SCHEMA}\toriginal_sha256={ORIGINAL_SOURCE_SHA256}\tprojection_sha256={PROJECTION_SHA256}"
        )?;
        writeln!(
            out,
            "row\tname\tsource_role\tclassification\tplanning_group\troute_contract\tgap"
        )?;
        for row in &self.rows {
            let route = if row.classification == Classification::NativeUsbDeclaration {
                CONTRACT
            } else {
                "none"
            };
            writeln!(
                out,
                "{}\t{}\t{:?}\t{:?}\t{:?}\t{}\t{}",
                row.ordinal, row.name, row.role, row.classification, row.group, route, row.gap
            )?;
        }
        Ok(())
    }

    /// Returns a declaration-only plan. There is deliberately no open/start API.
    pub fn plan(&self, request: &Request<'_>) -> Result<OfflineRoute, AdmissionError> {
        let row = self
            .rows
            .get(
                request
                    .capture_row
                    .checked_sub(1)
                    .ok_or(AdmissionError::UnknownRow)?,
            )
            .ok_or(AdmissionError::UnknownRow)?;
        if request.state != RuntimeState::Normal {
            return Err(AdmissionError::MaintenanceState);
        }
        if row.classification == Classification::MaintenanceLabelDenied {
            return Err(AdmissionError::MaintenanceState);
        }
        if request.endpoint_use != EndpointUse::Capture {
            return Err(AdmissionError::ControlOnlyEndpoint);
        }
        if row.role == SourceRole::Alias {
            return Err(AdmissionError::AliasOnlyIdentity);
        }
        if row.classification != Classification::NativeUsbDeclaration
            || request.capture_profile != ProductProfile::PodxtLive
        {
            return Err(AdmissionError::UnverifiedDevice);
        }
        if request.platform != Platform::Linux {
            return Err(AdmissionError::UnverifiedPlatform);
        }
        if request.contract != CONTRACT || request.installed_revision.is_some() {
            return Err(AdmissionError::UnverifiedVersion);
        }
        if request
            .connected_instrument
            .is_some_and(|p| p != ProductProfile::Variax600)
        {
            return Err(AdmissionError::WrongBridgeFamily);
        }
        if request.channels != pod_audio::DECLARED_CHANNELS
            || request.clock != (pod_audio::RATE_NUMERATOR, pod_audio::RATE_DENOMINATOR)
            || request.format != SampleFormat::S24ThreeByteLe
        {
            return Err(AdmissionError::UnsupportedPcm);
        }
        Ok(OfflineRoute {
            capture_row: row.ordinal,
            capture_profile: ProductProfile::PodxtLive,
            connected_instrument: request.connected_instrument,
            route_kind: if request.connected_instrument.is_some() {
                RouteKind::SpecificBridge
            } else {
                RouteKind::NativeUsb
            },
            declaration: PODXT_LIVE_DECLARATION,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Windows,
    MacOs,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeState {
    Normal,
    Maintenance,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointUse {
    Capture,
    ControlOnly,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    S24ThreeByteLe,
    S16Le,
    Float32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteKind {
    NativeUsb,
    SpecificBridge,
}

#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    pub capture_row: usize,
    pub capture_profile: ProductProfile,
    pub connected_instrument: Option<ProductProfile>,
    pub platform: Platform,
    pub state: RuntimeState,
    pub endpoint_use: EndpointUse,
    /// Version of the static source contract, not device firmware.
    pub contract: &'a str,
    /// No installed firmware revision has a validated audio contract here.
    pub installed_revision: Option<&'a str>,
    pub channels: u8,
    pub clock: (u32, u32),
    pub format: SampleFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    UnknownRow,
    MaintenanceState,
    ControlOnlyEndpoint,
    AliasOnlyIdentity,
    UnverifiedDevice,
    UnverifiedPlatform,
    UnverifiedVersion,
    WrongBridgeFamily,
    UnsupportedPcm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub source: &'static str,
    pub source_sha256: &'static str,
    pub routing_guide_sha256: &'static str,
    pub usb_vendor: u16,
    pub usb_product: u16,
    pub usb_interface: u8,
    pub capture_endpoint: u8,
    pub playback_endpoint: u8,
    pub platform: Platform,
    pub architecture: &'static str,
    pub capture_channels: u8,
    pub playback_channels: u8,
    pub clock: (u32, u32),
    pub format: SampleFormat,
    pub driver: &'static str,
    pub validation: &'static str,
    pub missing: &'static str,
    pub signal_and_monitoring: &'static str,
}

pub const PODXT_LIVE_DECLARATION: Declaration = Declaration {
    source: "Linux v7.2.15 sound/usb/line6/pod.c; GearBox2 Recording Rev03; docs/pod-usb-audio.md",
    source_sha256: "7b07e2e44d983b03abbf78351b09f7c9cdb3a6968d455c8fb14feaf1b73fa7b6",
    routing_guide_sha256: "0299bbb1d8c1c485adf33179deef2b2f424080ac8adcd83d687c1076efd029fa",
    usb_vendor: super::INITIAL_HOST.vendor_id,
    usb_product: super::INITIAL_HOST.product_id,
    usb_interface: 0,
    capture_endpoint: pod_audio::POD_EP_AUDIO_R,
    playback_endpoint: pod_audio::POD_EP_AUDIO_W,
    platform: Platform::Linux,
    architecture: "source declaration only; no per-architecture audio backend verified",
    capture_channels: 2,
    playback_channels: 2,
    clock: (pod_audio::RATE_NUMERATOR, pod_audio::RATE_DENOMINATOR),
    format: SampleFormat::S24ThreeByteLe,
    driver: "Linux snd-usb-pod; future replaceable ALSA adapter; no driver load/open performed",
    validation: "static source + documented routing + offline planner only, never live support",
    missing: "per-OS/architecture negotiation, installed firmware revision, live adapter and physical validation; no latency/permissions asserted",
    signal_and_monitoring: "GearBox Rev03 modes1/2 processed/clean send1-2, analog monitor includes host playback; AuxIn not captured. No automatic mode/monitor/reamp change.",
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfflineRoute {
    pub capture_row: usize,
    pub capture_profile: ProductProfile,
    pub connected_instrument: Option<ProductProfile>,
    pub route_kind: RouteKind,
    pub declaration: Declaration,
}

impl OfflineRoute {
    /// Uses the existing independent rig roles; the guitar is not the interface.
    pub fn connected_role(&self) -> RoleKind {
        if self.connected_instrument.is_some() {
            RoleKind::Guitar
        } else {
            RoleKind::Pod
        }
    }
}

/// Stable declaration export, separate from 224-row identity accounting.
pub fn write_declaration_tsv(mut out: impl Write) -> io::Result<()> {
    let d = PODXT_LIVE_DECLARATION;
    writeln!(
        out,
        "contract\tplatform\tarchitecture\tinterface_profile\tusb_vid_pid_interface\tcapture_endpoint\tplayback_endpoint\tcapture_channels\tplayback_channels\tclock\tformat\tdriver\tvalidation\tsource\tsource_sha256\tguide_sha256\tsignal_monitoring\tmissing"
    )?;
    writeln!(
        out,
        "{CONTRACT}\t{:?}\t{}\tPodxtLive\t{:04x}:{:04x}:{}\t{:02x}\t{:02x}\t{}\t{}\t{}/{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        d.platform,
        d.architecture,
        d.usb_vendor,
        d.usb_product,
        d.usb_interface,
        d.capture_endpoint,
        d.playback_endpoint,
        d.capture_channels,
        d.playback_channels,
        d.clock.0,
        d.clock.1,
        d.format,
        d.driver,
        d.validation,
        d.source,
        d.source_sha256,
        d.routing_guide_sha256,
        d.signal_and_monitoring,
        d.missing
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(registry: &Registry) -> Request<'static> {
        Request {
            capture_row: registry
                .rows()
                .iter()
                .find(|r| r.name == "PODxt Live")
                .unwrap()
                .ordinal,
            capture_profile: ProductProfile::PodxtLive,
            connected_instrument: None,
            platform: Platform::Linux,
            state: RuntimeState::Normal,
            endpoint_use: EndpointUse::Capture,
            contract: CONTRACT,
            installed_revision: None,
            channels: 2,
            clock: (78_125, 2),
            format: SampleFormat::S24ThreeByteLe,
        }
    }

    #[test]
    fn all_224_ordered_source_rows_remain_distinct_with_explicit_gaps() {
        let r = Registry::frozen().unwrap();
        assert_eq!(r.rows.len(), 224);
        assert_eq!(
            r.rows
                .iter()
                .filter(|r| r.role == SourceRole::Alias)
                .count(),
            186
        );
        for (index, row) in r.rows.iter().enumerate() {
            assert_eq!(row.ordinal, index + 1);
            assert!(!row.gap.is_empty());
        }
        let duplicates: Vec<_> = r.rows.iter().filter(|r| r.name == "TonePort").collect();
        assert_eq!(duplicates.len(), 2);
        assert_ne!(duplicates[0].ordinal, duplicates[1].ordinal);
        assert_eq!(
            r.rows
                .iter()
                .filter(|r| r.classification == Classification::NativeUsbDeclaration)
                .count(),
            1
        );
        assert_eq!(
            r.rows
                .iter()
                .filter(|r| r.classification == Classification::MaintenanceLabelDenied)
                .count(),
            3
        );
        assert_eq!(
            r.rows
                .iter()
                .filter(|r| r.classification == Classification::Unresolved)
                .count(),
            220
        );
    }

    #[test]
    fn changed_order_name_role_count_or_newline_is_input_drift() {
        for changed in [
            SNAPSHOT.replacen("Amplifi", "AMPLIFI", 1),
            SNAPSHOT.replacen("alias", "product", 1),
            SNAPSHOT.lines().skip(1).collect::<Vec<_>>().join("\n"),
            SNAPSHOT.replace('\n', "\r\n"),
            format!("{SNAPSHOT}extra\talias\n"),
        ] {
            assert!(Registry::from_snapshot(&changed).is_err());
        }
        assert!(Registry::from_projection(SNAPSHOT, 1, ORIGINAL_SOURCE_SHA256).is_err());
        assert!(Registry::from_projection(SNAPSHOT, SCHEMA, "different-original").is_err());
    }

    #[test]
    fn every_other_row_is_rejected_without_codec_or_alias_promotion() {
        let r = Registry::frozen().unwrap();
        let mut q = request(&r);
        for row in r.rows() {
            q.capture_row = row.ordinal;
            if row.classification == Classification::NativeUsbDeclaration {
                assert!(r.plan(&q).is_ok());
            } else {
                assert!(r.plan(&q).is_err(), "{} {}", row.ordinal, row.name);
            }
        }
        q.capture_row = 0;
        assert_eq!(r.plan(&q), Err(AdmissionError::UnknownRow));
        q.capture_row = 225;
        assert_eq!(r.plan(&q), Err(AdmissionError::UnknownRow));
    }

    #[test]
    fn capture_interface_and_connected_guitar_are_independent_rig_roles() {
        let r = Registry::frozen().unwrap();
        let mut q = request(&r);
        let native = r.plan(&q).unwrap();
        assert_eq!(native.route_kind, RouteKind::NativeUsb);
        assert_eq!(native.connected_role(), RoleKind::Pod);
        q.connected_instrument = Some(RoleKind::Guitar.profile());
        let bridge = r.plan(&q).unwrap();
        assert_eq!(bridge.capture_profile, RoleKind::Pod.profile());
        assert_eq!(bridge.connected_instrument, Some(ProductProfile::Variax600));
        assert_eq!(bridge.connected_role(), RoleKind::Guitar);
        assert_eq!(bridge.route_kind, RouteKind::SpecificBridge);
        assert_eq!(
            bridge.declaration.clock,
            (pod_audio::RATE_NUMERATOR, pod_audio::RATE_DENOMINATOR)
        );
        assert_eq!(
            bridge.declaration.usb_product,
            super::super::INITIAL_HOST.product_id
        );
        assert_eq!(bridge.declaration.usb_interface, 0);
        assert!(super::super::LIVE_BACKEND_INITIALIZER.is_none());
    }

    #[test]
    fn wrong_family_bridge_platform_revision_and_role_fail_closed() {
        let r = Registry::frozen().unwrap();
        let q = request(&r);
        for p in [
            ProductProfile::PodHd500x,
            ProductProfile::Jtv69,
            ProductProfile::WorkbenchHd,
            ProductProfile::Variax600,
        ] {
            assert_eq!(
                r.plan(&Request {
                    capture_profile: p,
                    ..q
                }),
                Err(AdmissionError::UnverifiedDevice)
            );
        }
        for p in [
            ProductProfile::Jtv69,
            ProductProfile::WorkbenchHd,
            ProductProfile::PodHd500x,
        ] {
            assert_eq!(
                r.plan(&Request {
                    connected_instrument: Some(p),
                    ..q
                }),
                Err(AdmissionError::WrongBridgeFamily)
            );
        }
        for platform in [Platform::Windows, Platform::MacOs] {
            assert_eq!(
                r.plan(&Request { platform, ..q }),
                Err(AdmissionError::UnverifiedPlatform)
            );
        }
        assert_eq!(
            r.plan(&Request {
                endpoint_use: EndpointUse::ControlOnly,
                ..q
            }),
            Err(AdmissionError::ControlOnlyEndpoint)
        );
        assert_eq!(
            r.plan(&Request {
                state: RuntimeState::Maintenance,
                ..q
            }),
            Err(AdmissionError::MaintenanceState)
        );
        assert_eq!(
            r.plan(&Request {
                contract: "other-version",
                ..q
            }),
            Err(AdmissionError::UnverifiedVersion)
        );
        assert_eq!(
            r.plan(&Request {
                installed_revision: Some("3.01"),
                ..q
            }),
            Err(AdmissionError::UnverifiedVersion)
        );
    }

    #[test]
    fn wrong_clock_channels_and_format_cannot_inherit_pod_contract() {
        let r = Registry::frozen().unwrap();
        let q = request(&r);
        for clock in [(39_062, 1), (39_063, 1), (44_100, 1), (48_000, 1)] {
            assert_eq!(
                r.plan(&Request { clock, ..q }),
                Err(AdmissionError::UnsupportedPcm)
            );
        }
        assert_eq!(
            r.plan(&Request { channels: 1, ..q }),
            Err(AdmissionError::UnsupportedPcm)
        );
        assert_eq!(
            r.plan(&Request {
                format: SampleFormat::Float32,
                ..q
            }),
            Err(AdmissionError::UnsupportedPcm)
        );
    }

    #[test]
    fn machine_exports_preserve_coverage_and_distinct_provenance() {
        let r = Registry::frozen().unwrap();
        let mut data = Vec::new();
        r.write_tsv(&mut data).unwrap();
        let text = String::from_utf8(data).unwrap();
        assert_eq!(text.lines().count(), 226);
        assert!(text.contains(ORIGINAL_SOURCE_SHA256) && text.contains(PROJECTION_SHA256));
        let mut declaration = Vec::new();
        write_declaration_tsv(&mut declaration).unwrap();
        let text = String::from_utf8(declaration).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("78125/2") && text.contains("source declaration only"));
    }

    #[test]
    fn explicit_planning_remainder_is_bounded_not_hidden_family_support() {
        let r = Registry::frozen().unwrap();
        let names: Vec<_> = r
            .rows()
            .iter()
            .filter(|r| r.group == WorkGroup::EffectsAndRecorders)
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "BackTrack",
                "BackTrack + Mic",
                "DL4 Mk",
                "DL4 MK2",
                "DL4 MK2 Bootloader",
                "DL4 MkII",
                "Echo Pro",
                "Filter Pro",
                "JM4",
                "M13",
                "M5",
                "M9",
                "Mod Pro"
            ]
        );
    }
}
