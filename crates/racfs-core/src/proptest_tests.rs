//! Property-based tests for racfs-core types (run with `cargo test`).

#![allow(unused_imports)]

use crate::flags::{OpenFlags, WriteFlags};
use proptest::prelude::*;

proptest! {
    /// Any u32 for OpenFlags: from_bits_truncate then .bits() roundtrips for defined bits.
    #[test]
    fn open_flags_bits_roundtrip(bits in 0u32..=0xFFFFu32) {
        let flags = OpenFlags::from_bits_retain(bits);
        // from_bits_retain keeps all bits; from_bits_truncate would drop unknown bits
        assert_eq!(flags.bits(), bits);
    }

    /// Any u32 for WriteFlags: bits roundtrip.
    #[test]
    fn write_flags_bits_roundtrip(bits in 0u32..=0xFFFFu32) {
        let flags = WriteFlags::from_bits_retain(bits);
        assert_eq!(flags.bits(), bits);
    }

    /// OpenFlags: is_read_only implies no WRITE bit.
    #[test]
    fn open_flags_read_only_no_write(bits in 0u32..=0xFFFFu32) {
        let flags = OpenFlags::from_bits_retain(bits);
        if flags.is_read_only() {
            assert!(!flags.contains(OpenFlags::WRITE));
        }
    }

    /// OpenFlags: is_read_write implies both READ and WRITE.
    #[test]
    fn open_flags_read_write_has_both(bits in 0u32..=0xFFFFu32) {
        let flags = OpenFlags::from_bits_retain(bits);
        if flags.is_read_write() {
            assert!(flags.contains(OpenFlags::READ));
            assert!(flags.contains(OpenFlags::WRITE));
        }
    }

    /// WriteFlags: contains_sync matches SYNC bit.
    #[test]
    fn write_flags_contains_sync(bits in 0u32..=0xFFFFu32) {
        let flags = WriteFlags::from_bits_retain(bits);
        assert_eq!(flags.contains_sync(), flags.contains(WriteFlags::SYNC));
    }

    /// WriteFlags: contains_append matches APPEND bit.
    #[test]
    fn write_flags_contains_append(bits in 0u32..=0xFFFFu32) {
        let flags = WriteFlags::from_bits_retain(bits);
        assert_eq!(flags.contains_append(), flags.contains(WriteFlags::APPEND));
    }
}
