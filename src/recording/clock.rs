//! Deterministic software-clock accounting for capture adapters.
//!
//! Observations are supplied by an adapter as monotonically increasing frame
//! positions and monotonic nanoseconds. This measures the adapter's software
//! observations against the declared rational source clock; it is not a device
//! timestamp, hardware synchronization, or latency measurement.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockError {
    InvalidRate,
    FirstFrameMustBeZero,
    NonMonotonicFrames,
    NonMonotonicTime,
    DropoutExceedsFrameDelta,
    ArithmeticOverflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockMeasurement {
    pub frame_index: u64,
    pub expected_elapsed_ns: u64,
    pub observed_elapsed_ns: u64,
    pub drift_ns: i128,
    /// `None` for the origin observation, whose elapsed time is zero.
    pub drift_ppm: Option<i64>,
    pub cumulative_dropout_frames: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareClockTracker {
    rate_num: u32,
    rate_den: u32,
    origin_ns: Option<u64>,
    previous_frame: Option<u64>,
    previous_ns: Option<u64>,
    cumulative_dropout_frames: u64,
    latest: Option<ClockMeasurement>,
}

impl SoftwareClockTracker {
    pub fn new(rate_num: u32, rate_den: u32) -> Result<Self, ClockError> {
        if rate_num == 0 || rate_den == 0 {
            return Err(ClockError::InvalidRate);
        }
        Ok(Self {
            rate_num,
            rate_den,
            origin_ns: None,
            previous_frame: None,
            previous_ns: None,
            cumulative_dropout_frames: 0,
            latest: None,
        })
    }

    pub const fn rate(&self) -> (u32, u32) {
        (self.rate_num, self.rate_den)
    }

    pub const fn latest(&self) -> Option<ClockMeasurement> {
        self.latest
    }

    /// Records an adapter-supplied software timestamp at a cumulative source
    /// frame position. The first observation establishes the origin at frame
    /// zero. `dropout_frames` counts known missing frames since the prior
    /// observation and is included in the cumulative source position.
    pub fn observe(
        &mut self,
        frame_index: u64,
        monotonic_ns: u64,
        dropout_frames: u64,
    ) -> Result<ClockMeasurement, ClockError> {
        let (Some(previous_frame), Some(previous_ns), Some(origin_ns)) =
            (self.previous_frame, self.previous_ns, self.origin_ns)
        else {
            if frame_index != 0 {
                return Err(ClockError::FirstFrameMustBeZero);
            }
            if dropout_frames != 0 {
                return Err(ClockError::DropoutExceedsFrameDelta);
            }
            self.origin_ns = Some(monotonic_ns);
            self.previous_frame = Some(0);
            self.previous_ns = Some(monotonic_ns);
            let measurement = ClockMeasurement {
                frame_index: 0,
                expected_elapsed_ns: 0,
                observed_elapsed_ns: 0,
                drift_ns: 0,
                drift_ppm: None,
                cumulative_dropout_frames: 0,
            };
            self.latest = Some(measurement);
            return Ok(measurement);
        };

        if frame_index <= previous_frame {
            return Err(ClockError::NonMonotonicFrames);
        }
        if monotonic_ns <= previous_ns {
            return Err(ClockError::NonMonotonicTime);
        }
        let frame_delta = frame_index - previous_frame;
        if dropout_frames > frame_delta {
            return Err(ClockError::DropoutExceedsFrameDelta);
        }

        let expected_elapsed_ns = self.frames_to_ns(frame_index)?;
        let observed_elapsed_ns = monotonic_ns
            .checked_sub(origin_ns)
            .ok_or(ClockError::NonMonotonicTime)?;
        let drift_ns = i128::from(observed_elapsed_ns) - i128::from(expected_elapsed_ns);
        let drift_ppm = if expected_elapsed_ns == 0 {
            None
        } else {
            let scaled = drift_ns
                .checked_mul(1_000_000)
                .ok_or(ClockError::ArithmeticOverflow)?;
            Some(
                i64::try_from(scaled / i128::from(expected_elapsed_ns))
                    .map_err(|_| ClockError::ArithmeticOverflow)?,
            )
        };
        let cumulative_dropout_frames = self
            .cumulative_dropout_frames
            .checked_add(dropout_frames)
            .ok_or(ClockError::ArithmeticOverflow)?;
        let measurement = ClockMeasurement {
            frame_index,
            expected_elapsed_ns,
            observed_elapsed_ns,
            drift_ns,
            drift_ppm,
            cumulative_dropout_frames,
        };
        self.previous_frame = Some(frame_index);
        self.previous_ns = Some(monotonic_ns);
        self.cumulative_dropout_frames = cumulative_dropout_frames;
        self.latest = Some(measurement);
        Ok(measurement)
    }

    /// Converts cumulative source frames using the exact rational clock and
    /// truncates only the sub-nanosecond remainder for the integer result.
    pub fn frames_to_ns(&self, frames: u64) -> Result<u64, ClockError> {
        let numerator = u128::from(frames)
            .checked_mul(u128::from(self.rate_den))
            .and_then(|value| value.checked_mul(1_000_000_000))
            .ok_or(ClockError::ArithmeticOverflow)?;
        let ns = numerator / u128::from(self.rate_num);
        u64::try_from(ns).map_err(|_| ClockError::ArithmeticOverflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rational_native_clock_accounts_observed_drift_and_dropout_frames() {
        let mut clock = SoftwareClockTracker::new(78_125, 2).unwrap();
        assert_eq!(clock.frames_to_ns(390_625), Ok(10_000_000_000));
        assert_eq!(clock.observe(0, 5_000, 0).unwrap().drift_ppm, None);
        let measurement = clock.observe(390_625, 10_000_005_000, 10).unwrap();
        assert_eq!(measurement.expected_elapsed_ns, 10_000_000_000);
        assert_eq!(measurement.observed_elapsed_ns, 10_000_000_000);
        assert_eq!(measurement.drift_ns, 0);
        assert_eq!(measurement.drift_ppm, Some(0));
        assert_eq!(measurement.cumulative_dropout_frames, 10);
    }

    #[test]
    fn drift_scenarios_use_explicit_software_observations() {
        let mut clock = SoftwareClockTracker::new(48_000, 1).unwrap();
        clock.observe(0, 1_000, 0).unwrap();
        let fast = clock.observe(48_000, 999_001_000, 0).unwrap();
        assert_eq!(fast.expected_elapsed_ns, 1_000_000_000);
        assert_eq!(fast.drift_ns, -1_000_000);
        assert_eq!(fast.drift_ppm, Some(-1_000));

        let slow = clock.observe(96_000, 2_001_001_000, 24).unwrap();
        assert_eq!(slow.expected_elapsed_ns, 2_000_000_000);
        assert_eq!(slow.observed_elapsed_ns, 2_001_000_000);
        assert_eq!(slow.drift_ppm, Some(500));
        assert_eq!(slow.cumulative_dropout_frames, 24);
    }

    #[test]
    fn invalid_observations_do_not_mutate_the_last_measurement() {
        let mut clock = SoftwareClockTracker::new(44_100, 1).unwrap();
        let origin = clock.observe(0, 100, 0).unwrap();
        assert_eq!(
            clock.observe(0, 200, 0),
            Err(ClockError::NonMonotonicFrames)
        );
        assert_eq!(clock.latest(), Some(origin));
        let next = clock.observe(44_100, 1_000_000_100, 0).unwrap();
        assert_eq!(next.expected_elapsed_ns, 1_000_000_000);
        assert_eq!(
            clock.observe(44_200, 1_000_001_100, 101),
            Err(ClockError::DropoutExceedsFrameDelta)
        );
        assert_eq!(clock.latest(), Some(next));
        assert_eq!(
            clock.observe(44_201, 1_000_000_100, 0),
            Err(ClockError::NonMonotonicTime)
        );
    }

    #[test]
    fn invalid_rates_and_arithmetic_overflow_fail_closed() {
        assert_eq!(
            SoftwareClockTracker::new(0, 1),
            Err(ClockError::InvalidRate)
        );
        assert_eq!(
            SoftwareClockTracker::new(1, 0),
            Err(ClockError::InvalidRate)
        );
        let slow = SoftwareClockTracker::new(1, 1).unwrap();
        assert_eq!(
            slow.frames_to_ns(u64::MAX),
            Err(ClockError::ArithmeticOverflow)
        );
    }
}
