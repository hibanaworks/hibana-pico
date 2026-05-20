pub(crate) const NUM_MATRIX_LEDS: usize = 104;
pub(crate) const MATRIX_BYTES: usize = 13;

// ArduinoCore-zephyr's UNO Q loader uses this physical charlieplex order for
// the onboard 13x8 matrix. The logical bitmap order is row-major; this table
// translates logical bit index to GPIOF high/low pins.
pub(crate) const CHARLIE_PAIRS: [(u8, u8); NUM_MATRIX_LEDS] = [
    (0, 1),
    (1, 0),
    (0, 2),
    (2, 0),
    (1, 2),
    (2, 1),
    (0, 3),
    (3, 0),
    (1, 3),
    (3, 1),
    (2, 3),
    (3, 2),
    (0, 4),
    (4, 0),
    (1, 4),
    (4, 1),
    (2, 4),
    (4, 2),
    (3, 4),
    (4, 3),
    (0, 5),
    (5, 0),
    (1, 5),
    (5, 1),
    (2, 5),
    (5, 2),
    (3, 5),
    (5, 3),
    (4, 5),
    (5, 4),
    (0, 6),
    (6, 0),
    (1, 6),
    (6, 1),
    (2, 6),
    (6, 2),
    (3, 6),
    (6, 3),
    (4, 6),
    (6, 4),
    (5, 6),
    (6, 5),
    (0, 7),
    (7, 0),
    (1, 7),
    (7, 1),
    (2, 7),
    (7, 2),
    (3, 7),
    (7, 3),
    (4, 7),
    (7, 4),
    (5, 7),
    (7, 5),
    (6, 7),
    (7, 6),
    (0, 8),
    (8, 0),
    (1, 8),
    (8, 1),
    (2, 8),
    (8, 2),
    (3, 8),
    (8, 3),
    (4, 8),
    (8, 4),
    (5, 8),
    (8, 5),
    (6, 8),
    (8, 6),
    (7, 8),
    (8, 7),
    (0, 9),
    (9, 0),
    (1, 9),
    (9, 1),
    (2, 9),
    (9, 2),
    (3, 9),
    (9, 3),
    (4, 9),
    (9, 4),
    (5, 9),
    (9, 5),
    (6, 9),
    (9, 6),
    (7, 9),
    (9, 7),
    (8, 9),
    (9, 8),
    (0, 10),
    (10, 0),
    (1, 10),
    (10, 1),
    (2, 10),
    (10, 2),
    (3, 10),
    (10, 3),
    (4, 10),
    (10, 4),
    (5, 10),
    (10, 5),
    (6, 10),
    (10, 6),
];

pub(crate) fn matrix_bit(bytes: &[u8; MATRIX_BYTES], index: usize) -> bool {
    if index >= NUM_MATRIX_LEDS {
        return false;
    }
    ((bytes[index / 8] >> (index % 8)) & 1) != 0
}

pub(crate) fn next_lit_index(bytes: &[u8; MATRIX_BYTES], start: usize) -> Option<usize> {
    let mut offset = 0usize;
    while offset < NUM_MATRIX_LEDS {
        let index = (start + offset) % NUM_MATRIX_LEDS;
        if matrix_bit(bytes, index) {
            return Some(index);
        }
        offset += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{CHARLIE_PAIRS, MATRIX_BYTES, NUM_MATRIX_LEDS, next_lit_index};

    #[test]
    fn uno_q_matrix_uses_official_physical_pair_order() {
        assert_eq!(CHARLIE_PAIRS.len(), NUM_MATRIX_LEDS);
        assert_eq!(
            &CHARLIE_PAIRS[..12],
            &[
                (0, 1),
                (1, 0),
                (0, 2),
                (2, 0),
                (1, 2),
                (2, 1),
                (0, 3),
                (3, 0),
                (1, 3),
                (3, 1),
                (2, 3),
                (3, 2),
            ]
        );
        assert_eq!(
            &CHARLIE_PAIRS[90..],
            &[
                (0, 10),
                (10, 0),
                (1, 10),
                (10, 1),
                (2, 10),
                (10, 2),
                (3, 10),
                (10, 3),
                (4, 10),
                (10, 4),
                (5, 10),
                (10, 5),
                (6, 10),
                (10, 6),
            ]
        );
    }

    #[test]
    fn uno_q_matrix_rejects_generic_nested_gpio_order() {
        let mut generic = [(0u8, 1u8); NUM_MATRIX_LEDS];
        let mut out = 0usize;
        let mut high = 0u8;
        while high < 11 {
            let mut low = 0u8;
            while low < 11 {
                if high != low && out < NUM_MATRIX_LEDS {
                    generic[out] = (high, low);
                    out += 1;
                }
                low += 1;
            }
            high += 1;
        }

        assert_ne!(CHARLIE_PAIRS, generic);
    }

    #[test]
    fn uno_q_matrix_scan_skips_dark_slots_and_wraps() {
        let mut bitmap = [0u8; MATRIX_BYTES];
        bitmap[5 / 8] |= 1 << (5 % 8);
        bitmap[90 / 8] |= 1 << (90 % 8);

        assert_eq!(next_lit_index(&bitmap, 0), Some(5));
        assert_eq!(next_lit_index(&bitmap, 6), Some(90));
        assert_eq!(next_lit_index(&bitmap, 91), Some(5));
        assert_eq!(next_lit_index(&[0; MATRIX_BYTES], 0), None);
    }
}
