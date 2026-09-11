//! Time model: rational timestamps in a track's timebase.
//!
//! Every stored time is a `Timestamp { num, den }`, meaning `num / den`
//! seconds. Rationals avoid drift over hour-long videos with odd frame rates
//! (29.97, 23.976). Ranges are half-open `[t0, t1)`.

use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

/// A point in time as a rational number of seconds: `num / den`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Timestamp {
    /// Numerator; may be negative for pre-roll.
    pub num: i64,
    /// Denominator; never zero.
    pub den: u32,
}

impl Timestamp {
    /// Denominator used when a timestamp has no natural timebase (microseconds,
    /// matching libav's `AV_TIME_BASE`).
    pub const MICROS: u32 = 1_000_000;

    /// Zero.
    pub const ZERO: Timestamp = Timestamp { num: 0, den: 1 };

    /// Construct from numerator and denominator. Panics only in debug builds
    /// when `den == 0`; release builds substitute `den = 1`.
    pub const fn new(num: i64, den: u32) -> Self {
        debug_assert!(den != 0, "Timestamp denominator must be non-zero");
        let den = if den == 0 { 1 } else { den };
        Self { num, den }
    }

    /// Whole seconds.
    pub const fn from_secs(secs: i64) -> Self {
        Self { num: secs, den: 1 }
    }

    /// Microseconds.
    pub const fn from_micros(us: i64) -> Self {
        Self {
            num: us,
            den: Self::MICROS,
        }
    }

    /// Nearest representable value at the given denominator.
    pub fn from_secs_f64(secs: f64, den: u32) -> Self {
        let den = den.max(1);
        Self {
            num: (secs * f64::from(den)).round() as i64,
            den,
        }
    }

    /// Seconds as a float, for indexing and display.
    pub fn as_secs_f64(&self) -> f64 {
        self.num as f64 / f64::from(self.den)
    }

    /// Microseconds, rounded.
    pub fn as_micros(&self) -> i64 {
        self.rescale(Self::MICROS).num
    }

    /// Re-express in another denominator, rounding to nearest.
    pub fn rescale(&self, den: u32) -> Self {
        if self.den == den {
            return *self;
        }
        let den = den.max(1);
        let num = i128::from(self.num) * i128::from(den);
        let d = i128::from(self.den);
        // Round half away from zero.
        let rounded = if num >= 0 {
            (num + d / 2) / d
        } else {
            (num - d / 2) / d
        };
        Self {
            num: rounded.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
            den,
        }
    }

    /// Saturating addition. The result uses the least common multiple of the
    /// two denominators when it fits in `u32`, so no precision is lost;
    /// otherwise microseconds.
    pub fn add(&self, other: Timestamp) -> Self {
        let den = common_den(self.den, other.den);
        let a = self.rescale(den);
        let b = other.rescale(den);
        Self {
            num: a.num.saturating_add(b.num),
            den,
        }
    }

    /// Saturating subtraction, see [`Timestamp::add`].
    pub fn sub(&self, other: Timestamp) -> Self {
        let den = common_den(self.den, other.den);
        let a = self.rescale(den);
        let b = other.rescale(den);
        Self {
            num: a.num.saturating_sub(b.num),
            den,
        }
    }

    /// True when `self < 0`.
    pub fn is_negative(&self) -> bool {
        self.num < 0
    }

    fn cross(&self, other: &Timestamp) -> (i128, i128) {
        (
            i128::from(self.num) * i128::from(other.den),
            i128::from(other.num) * i128::from(self.den),
        )
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// LCM of two denominators if it fits in `u32`, else microseconds.
fn common_den(a: u32, b: u32) -> u32 {
    if a == b {
        return a;
    }
    let (a64, b64) = (u64::from(a.max(1)), u64::from(b.max(1)));
    let l = a64 / gcd(a64, b64) * b64;
    u32::try_from(l).unwrap_or(Timestamp::MICROS)
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::ZERO
    }
}

impl PartialEq for Timestamp {
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = self.cross(other);
        a == b
    }
}

impl Eq for Timestamp {}

impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        let (a, b) = self.cross(other);
        a.cmp(&b)
    }
}

impl std::hash::Hash for Timestamp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash a canonical form so equal rationals hash equally.
        self.as_micros().hash(state);
    }
}

impl fmt::Display for Timestamp {
    /// `HH:MM:SS.mmm`; negative values get a leading `-`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total_ms = self.rescale(1000).num;
        let sign = if total_ms < 0 { "-" } else { "" };
        let ms = total_ms.unsigned_abs();
        let h = ms / 3_600_000;
        let m = (ms / 60_000) % 60;
        let s = (ms / 1000) % 60;
        let milli = ms % 1000;
        write!(f, "{sign}{h:02}:{m:02}:{s:02}.{milli:03}")
    }
}

/// Half-open time range `[t0, t1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    /// Inclusive start.
    pub t0: Timestamp,
    /// Exclusive end.
    pub t1: Timestamp,
}

impl TimeRange {
    /// Build a range; returns `None` if `t1 <= t0`.
    pub fn new(t0: Timestamp, t1: Timestamp) -> Option<Self> {
        (t1 > t0).then_some(Self { t0, t1 })
    }

    /// Duration as a timestamp.
    pub fn duration(&self) -> Timestamp {
        self.t1.sub(self.t0)
    }

    /// True if `t0 <= t < t1`.
    pub fn contains(&self, t: Timestamp) -> bool {
        t >= self.t0 && t < self.t1
    }

    /// True if the ranges share any instant.
    pub fn overlaps(&self, other: &TimeRange) -> bool {
        self.t0 < other.t1 && other.t0 < self.t1
    }
}

impl fmt::Display for TimeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {})", self.t0, self.t1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality_across_denominators() {
        let a = Timestamp::new(1, 2);
        let b = Timestamp::new(500, 1000);
        assert_eq!(a, b);
        assert_eq!(a.cmp(&b), Ordering::Equal);
        assert!(Timestamp::new(1001, 30000) > Timestamp::new(1, 30));
    }

    #[test]
    fn rescale_rounds_to_nearest() {
        let t = Timestamp::new(1, 3); // 0.333...
        assert_eq!(t.rescale(1000).num, 333);
        let t = Timestamp::new(2, 3); // 0.666...
        assert_eq!(t.rescale(1000).num, 667);
        let t = Timestamp::new(-2, 3);
        assert_eq!(t.rescale(1000).num, -667);
    }

    #[test]
    fn no_drift_at_ntsc_rates() {
        // 1 hour of 29.97 fps frames in timebase 1/30000, frame duration 1001.
        let frames: i64 = 3600 * 30000 / 1001;
        let t = Timestamp::new(frames * 1001, 30000);
        // 107892 frames * 1001 / 30000 = 3599.9964 s exactly.
        assert_eq!(frames, 107_892);
        assert_eq!(t.rescale(10_000).num, 35_999_964);
        assert_eq!(t.to_string(), "00:59:59.996");
        // Summing one frame duration 107892 times must give the same value.
        let step = Timestamp::new(1001, 30000);
        let mut acc = Timestamp::new(0, 30000);
        for _ in 0..frames {
            acc = acc.add(step);
        }
        assert_eq!(acc, t);
    }

    #[test]
    fn display_format() {
        assert_eq!(Timestamp::from_secs(3661).to_string(), "01:01:01.000");
        assert_eq!(Timestamp::new(-1500, 1000).to_string(), "-00:00:01.500");
        assert_eq!(
            Timestamp::from_secs_f64(1834.2, 1000).to_string(),
            "00:30:34.200"
        );
    }

    #[test]
    fn arithmetic_uses_finer_denominator() {
        let a = Timestamp::new(1, 2);
        let b = Timestamp::new(1, 3);
        let s = a.add(b);
        assert_eq!(s.den, 6);
        assert_eq!(s.num, 5);
        // Denominators whose LCM overflows u32 fall back to microseconds.
        let big = Timestamp::new(1, 4_000_000_000).add(Timestamp::new(1, 3_999_999_999));
        assert_eq!(big.den, Timestamp::MICROS);
    }

    #[test]
    fn ranges_are_half_open() {
        let r = TimeRange::new(Timestamp::from_secs(10), Timestamp::from_secs(20)).unwrap();
        assert!(r.contains(Timestamp::from_secs(10)));
        assert!(!r.contains(Timestamp::from_secs(20)));
        assert!(TimeRange::new(Timestamp::from_secs(5), Timestamp::from_secs(5)).is_none());
        let other = TimeRange::new(Timestamp::from_secs(20), Timestamp::from_secs(30)).unwrap();
        assert!(!r.overlaps(&other));
    }

    #[test]
    fn serde_roundtrip() {
        let t = Timestamp::new(1001, 30000);
        let s = serde_json::to_string(&t).unwrap();
        assert_eq!(s, r#"{"num":1001,"den":30000}"#);
        let back: Timestamp = serde_json::from_str(&s).unwrap();
        assert_eq!(back, t);
    }
}
