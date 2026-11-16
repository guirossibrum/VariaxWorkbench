use super::{ModelError, Profile};

/// Stages evidenced for the legacy Variax normal/boot receive paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    MainCode,
    BootRegion,
    UserPatches,
    FactoryTemplates,
    EraseUser,
    EraseFactory,
}

/// Explicit scenario assumption, never detected from a physical device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatingMode {
    Application,
    BootCode,
}

impl Stage {
    pub(super) const fn command(self) -> u8 {
        match self {
            Self::MainCode => 0x60,
            Self::BootRegion => 0x6f,
            Self::UserPatches => 0x62,
            Self::FactoryTemplates => 0x67,
            Self::EraseUser => 0x63,
            Self::EraseFactory => 0x68,
        }
    }

    const fn is_erase(self) -> bool {
        matches!(self, Self::EraseUser | Self::EraseFactory)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Header,
    Count,
    Payload,
    /// Conditional normal-path reply; peripheral helpers are not modeled.
    ModeledReply,
    Rejected,
    Interrupted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Integrity {
    NotChecked,
    Additive24Matched,
    AdditiveByteMatched,
    NoPacketChecksum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Timeout,
    Disconnect,
    PowerLoss,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiverReport {
    phase: Phase,
    stage: Stage,
    mode: OperatingMode,
    received_nibbles: usize,
    /// Calls to programming-like helpers, not a physical write counter.
    programming_intent_bytes: usize,
    /// Explicit erase-command handler entered before its terminator check.
    /// Does not describe erase operations inside programming helpers or prior stages.
    erase_intent: bool,
    integrity: Integrity,
    /// Derived response command, not bytes returned by a connected device.
    modeled_acknowledgment: Option<u8>,
    fault: Option<Fault>,
    rejection: Option<ModelError>,
}

impl ReceiverReport {
    pub const fn phase(&self) -> Phase {
        self.phase
    }
    pub const fn stage(&self) -> Stage {
        self.stage
    }
    pub const fn mode(&self) -> OperatingMode {
        self.mode
    }
    pub const fn received_nibbles(&self) -> usize {
        self.received_nibbles
    }
    pub const fn programming_intent_bytes(&self) -> usize {
        self.programming_intent_bytes
    }
    /// Explicit erase command only; false does not mean no physical erase.
    pub const fn erase_intent(&self) -> bool {
        self.erase_intent
    }
    pub const fn integrity(&self) -> Integrity {
        self.integrity
    }
    pub const fn modeled_acknowledgment(&self) -> Option<u8> {
        self.modeled_acknowledgment
    }
    pub const fn fault(&self) -> Option<Fault> {
        self.fault
    }
    pub const fn rejection(&self) -> Option<&ModelError> {
        self.rejection.as_ref()
    }

    /// Intent observed within this selected stage, not proof of device state.
    /// False cannot exclude earlier stages or unmodeled peripheral effects.
    pub const fn possible_persistent_change(&self) -> bool {
        self.erase_intent || self.programming_intent_bytes > 0
    }

    pub const fn durability_known(&self) -> bool {
        false
    }

    pub const fn hardware_safe(&self) -> bool {
        false
    }

    pub const fn peripheral_helpers_modeled(&self) -> bool {
        false
    }
}

/// Limits are application bounds for synthetic experiments, not recovered RAM
/// or compatibility limits. Zero-payload code streams remain unsupported.
pub const MAX_MAIN_WORDS: usize = 32_768;
pub const MAX_FEED_BYTES: usize = 8192;
const USER_WIRE_BYTES: usize = 3 * 65_536;
const FACTORY_BYTES: usize = 50 * 2619;
const BOOT_BYTES: usize = 16_384;

/// A bounded counter/buffer model. It has no flash image, device handle, writer,
/// OS adapter, sleep, timer or recovery implementation. A selected stage is an
/// explicit scenario assumption; constructing it does not model boot entry.
/// Wrong-stage/header/count/noncanonical guards are model admission policies,
/// not proof that original hardware would reject such traffic without effects.
pub struct Receiver {
    report: ReceiverReport,
    header_bytes: usize,
    count: Vec<u8>,
    nibbles: Vec<u8>,
    expected_nibbles: Option<usize>,
    boot_operand_seen: bool,
}

impl Receiver {
    pub fn new(profile: Profile, stage: Stage) -> Result<Self, ModelError> {
        Self::in_mode(profile, OperatingMode::Application, stage)
    }

    pub fn in_mode(
        profile: Profile,
        mode: OperatingMode,
        stage: Stage,
    ) -> Result<Self, ModelError> {
        if profile != Profile::Variax600 {
            return Err(ModelError::WrongFamily);
        }
        if mode == OperatingMode::BootCode && !matches!(stage, Stage::MainCode | Stage::BootRegion)
        {
            // Boot code has a different single-sector erase command. Do not
            // invent full-bank erase or preset reception in that mode.
            return Err(ModelError::UnsupportedOperation);
        }
        let expected_nibbles = match stage {
            Stage::MainCode => None,
            Stage::BootRegion => Some((BOOT_BYTES + 1) * 2),
            Stage::UserPatches => Some(USER_WIRE_BYTES * 2),
            Stage::FactoryTemplates => Some(FACTORY_BYTES * 2),
            Stage::EraseUser | Stage::EraseFactory => Some(0),
        };
        Ok(Self {
            report: ReceiverReport {
                phase: Phase::Header,
                stage,
                mode,
                received_nibbles: 0,
                programming_intent_bytes: 0,
                erase_intent: false,
                integrity: Integrity::NotChecked,
                modeled_acknowledgment: None,
                fault: None,
                rejection: None,
            },
            header_bytes: 0,
            count: Vec::new(),
            nibbles: Vec::new(),
            expected_nibbles,
            boot_operand_seen: false,
        })
    }

    pub fn report(&self) -> ReceiverReport {
        self.report.clone()
    }

    fn reject<T>(&mut self, error: ModelError) -> Result<T, ModelError> {
        self.report.phase = Phase::Rejected;
        self.report.rejection = Some(error.clone());
        Err(error)
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), ModelError> {
        if bytes.len() > MAX_FEED_BYTES {
            return self.reject(ModelError::Limit);
        }
        for &byte in bytes {
            // MIDI realtime interleaving is modeled separately from data nibbles.
            if byte >= 0xf8 {
                continue;
            }
            if self.report.phase == Phase::Interrupted {
                return Err(ModelError::Interrupted);
            }
            if self.report.phase == Phase::Rejected {
                return Err(self
                    .report
                    .rejection
                    .clone()
                    .unwrap_or(ModelError::ExtraData));
            }
            if self.report.phase == Phase::ModeledReply {
                return self.reject(ModelError::ExtraData);
            }
            if self.header_bytes < 6 {
                if byte != crate::HEADER[self.header_bytes] {
                    return self.reject(ModelError::InvalidHeader);
                }
                self.header_bytes += 1;
                continue;
            }
            if self.header_bytes == 6 {
                if byte != self.report.stage.command() {
                    return self.reject(ModelError::WrongStage);
                }
                self.header_bytes += 1;
                if self.report.stage.is_erase() {
                    self.report.erase_intent = true;
                }
                self.report.phase = if self.report.stage == Stage::MainCode {
                    Phase::Count
                } else {
                    Phase::Payload
                };
                continue;
            }
            if self.report.stage == Stage::BootRegion && !self.boot_operand_seen {
                if byte != 2 {
                    return self.reject(ModelError::UnsupportedOperation);
                }
                self.boot_operand_seen = true;
                continue;
            }
            if self.report.phase == Phase::Count {
                if byte > 15 {
                    return self.reject(ModelError::NonCanonicalNibble);
                }
                self.count.push(byte);
                if self.count.len() == 6 {
                    let words = word(&self.count) as usize;
                    if !(2..=MAX_MAIN_WORDS + 1).contains(&words) {
                        return self.reject(ModelError::UnsupportedCount);
                    }
                    self.expected_nibbles = Some(words * 6);
                    self.report.phase = Phase::Payload;
                }
                continue;
            }
            if byte == 0xf7 {
                self.complete()?;
                continue;
            }
            if byte > 15 {
                // Original paths sometimes mask values. Their noncanonical
                // semantics are deliberately not invented by this model.
                return self.reject(ModelError::NonCanonicalNibble);
            }
            if self.nibbles.len() >= self.expected_nibbles.unwrap_or(0) {
                return self.reject(ModelError::ExtraData);
            }
            self.nibbles.push(byte);
            self.report.received_nibbles = self.nibbles.len();
            if self.nibbles.len().is_multiple_of(2) {
                let at = self.nibbles.len() / 2 - 1;
                let word_completed = match self.report.stage {
                    Stage::FactoryTemplates => (at + 1).is_multiple_of(3),
                    Stage::UserPatches => {
                        let group_offset = at % 65_536;
                        group_offset < 65_520 && (group_offset + 1).is_multiple_of(3)
                    }
                    _ => false,
                };
                if word_completed {
                    self.report.programming_intent_bytes += 3;
                }
            }
        }
        Ok(())
    }

    fn complete(&mut self) -> Result<(), ModelError> {
        if self.nibbles.len() != self.expected_nibbles.unwrap_or(usize::MAX) {
            return self.reject(ModelError::Truncated);
        }
        match self.report.stage {
            Stage::MainCode => {
                let checksum_at = self.nibbles.len() - 6;
                let sum = self.nibbles[..checksum_at]
                    .as_chunks::<6>()
                    .0
                    .iter()
                    .fold(0u32, |sum, item| sum.wrapping_add(word(item)))
                    & 0xff_ffff;
                if sum != word(&self.nibbles[checksum_at..]) {
                    return self.reject(ModelError::Checksum);
                }
                self.report.integrity = Integrity::Additive24Matched;
                self.report.programming_intent_bytes = checksum_at / 2;
            }
            Stage::BootRegion => {
                let sum = self.nibbles[..BOOT_BYTES * 2]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .fold(0u8, |sum, pair| sum.wrapping_add((pair[0] << 4) | pair[1]));
                let tail = &self.nibbles[BOOT_BYTES * 2..];
                if sum != (tail[0] << 4) | tail[1] {
                    return self.reject(ModelError::Checksum);
                }
                self.report.integrity = Integrity::AdditiveByteMatched;
                self.report.programming_intent_bytes = BOOT_BYTES;
            }
            _ => self.report.integrity = Integrity::NoPacketChecksum,
        }
        self.report.modeled_acknowledgment = Some(if self.report.stage.is_erase() {
            0x70
        } else {
            0x71
        });
        self.report.phase = Phase::ModeledReply;
        Ok(())
    }

    pub fn finish(&mut self) -> Result<ReceiverReport, ModelError> {
        match self.report.phase {
            Phase::ModeledReply => Ok(self.report()),
            Phase::Rejected => Err(self
                .report
                .rejection
                .clone()
                .unwrap_or(ModelError::Truncated)),
            Phase::Interrupted => Err(ModelError::Interrupted),
            _ => self.reject(ModelError::Truncated),
        }
    }

    pub fn interrupt(&mut self, fault: Fault) {
        self.report.fault = Some(fault);
        self.report.phase = Phase::Interrupted;
        // Deliberately retain prior intent and buffers. No rollback is assumed.
    }

    pub fn recover(&mut self) -> Result<(), ModelError> {
        Err(ModelError::UnsupportedRecovery)
    }
}

fn word(nibbles: &[u8]) -> u32 {
    nibbles
        .iter()
        .rev()
        .fold(0u32, |word, &n| (word << 4) | u32::from(n))
}
