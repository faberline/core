//! UUIDv7 maker (RFC 9562) with an injectable clock and random source.
//!
//! The 12-bit counter lives in `rand_a`, so ids made in one millisecond still differ and sort.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Makes UUIDv7 text ids. `clock` returns Unix milliseconds; `random` returns 8 random bytes.
pub struct UuidV7Maker<C, R> {
    clock: C,
    random: R,
    last_ms: Option<u64>,
    seq: u16,
}

impl<C: FnMut() -> u64, R: FnMut() -> [u8; 8]> UuidV7Maker<C, R> {
    /// Create a maker from a clock and a random source.
    pub fn new(clock: C, random: R) -> Self {
        Self {
            clock,
            random,
            last_ms: None,
            seq: 0,
        }
    }

    /// Make the next id, as 36 lowercase characters in the `8-4-4-4-12` form.
    pub fn next_id(&mut self) -> String {
        let now = (self.clock)();
        let (ms, seq) = match self.last_ms {
            Some(last) => {
                let ms = now.max(last);
                if ms == last {
                    (ms, (self.seq + 1) & 0x0FFF)
                } else {
                    (ms, 0)
                }
            }
            None => (now, 0),
        };
        self.last_ms = Some(ms);
        self.seq = seq;
        let random = (self.random)();
        format_uuid(&build_bytes(ms, seq, random))
    }
}

impl UuidV7Maker<fn() -> u64, fn() -> [u8; 8]> {
    /// A maker that reads the system clock and the process random source.
    pub fn system() -> Self {
        Self::new(
            system_clock as fn() -> u64,
            system_random as fn() -> [u8; 8],
        )
    }
}

/// Lay out the 16 bytes of a UUIDv7: 48-bit time, version, 12-bit counter, variant, random.
fn build_bytes(ms: u64, seq: u16, random: [u8; 8]) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[..6].copy_from_slice(&ms.to_be_bytes()[2..]);
    b[6] = 0x70 | (((seq >> 8) as u8) & 0x0F);
    b[7] = (seq & 0xFF) as u8;
    b[8] = 0x80 | (random[0] & 0x3F);
    b[9..].copy_from_slice(&random[1..]);
    b
}

/// Write the 16 bytes as lowercase hex in the `8-4-4-4-12` form.
fn format_uuid(b: &[u8; 16]) -> String {
    let mut text = String::with_capacity(36);
    for (i, byte) in b.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

fn system_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn system_random() -> [u8; 8] {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(n);
    hasher.finish().to_be_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 0x0123_4567_89AB;

    fn fixed_maker() -> UuidV7Maker<impl FnMut() -> u64, impl FnMut() -> [u8; 8]> {
        UuidV7Maker::new(|| MS, || [0xFF; 8])
    }

    #[test]
    fn id_has_version_7_variant_and_time() {
        let mut maker = fixed_maker();
        let id = maker.next_id();
        assert_eq!(id.len(), 36);
        for dash in [8, 13, 18, 23] {
            assert_eq!(id.as_bytes()[dash], b'-');
        }
        assert_eq!(id.as_bytes()[14], b'7');
        assert!(matches!(id.as_bytes()[19], b'8' | b'9' | b'a' | b'b'));
        assert_eq!(&id.replace('-', "")[..12], "0123456789ab");
    }

    #[test]
    fn ids_in_the_same_millisecond_differ() {
        let mut maker = fixed_maker();
        let first = maker.next_id();
        let second = maker.next_id();
        assert_ne!(first, second);
        assert_eq!(&first[15..18], "000");
        assert_eq!(&second[15..18], "001");
    }

    #[test]
    fn new_millisecond_resets_the_counter() {
        let times = [1u64, 1, 2];
        let mut index = 0;
        let mut maker = UuidV7Maker::new(
            move || {
                let t = times[index];
                index += 1;
                t
            },
            || [0xFF; 8],
        );
        let ids: Vec<String> = (0..3).map(|_| maker.next_id()).collect();
        assert_eq!(&ids[0][15..18], "000");
        assert_eq!(&ids[1][15..18], "001");
        assert_eq!(&ids[2][15..18], "000");
    }

    #[test]
    fn clock_going_back_keeps_the_last_time() {
        let times = [5u64, 3];
        let mut index = 0;
        let mut maker = UuidV7Maker::new(
            move || {
                let t = times[index];
                index += 1;
                t
            },
            || [0xFF; 8],
        );
        let first = maker.next_id();
        let second = maker.next_id();
        assert_eq!(&first.replace('-', "")[..12], "000000000005");
        assert_eq!(&second.replace('-', "")[..12], "000000000005");
    }

    #[test]
    fn system_maker_makes_a_valid_shape() {
        let mut maker = UuidV7Maker::system();
        let first = maker.next_id();
        let second = maker.next_id();
        assert_eq!(first.len(), 36);
        assert_eq!(first.as_bytes()[14], b'7');
        assert_ne!(first, second);
    }
}
