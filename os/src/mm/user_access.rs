//! Checked user-memory access. References: official Guide chapter 4; AI-assisted implementation.
use super::{translated_byte_buffer, PTEFlags, PageTable, VirtAddr};

/// Check the entire range before reading or modifying any user memory.
pub fn user_buffer_valid(token: usize, start: usize, len: usize, write: bool) -> bool {
    let end = match start.checked_add(len) {
        Some(end) if end <= (1usize << 38) => end,
        _ => return false,
    };
    if start >= (1usize << 38) {
        return false;
    }
    if len == 0 {
        return true;
    }
    let table = PageTable::from_token(token);
    let first = VirtAddr::from(start).floor();
    let last = VirtAddr::from(end - 1).floor();
    (first.0..=last.0).all(|vpn| {
        table.translate(vpn.into()).map_or(false, |pte| {
            pte.is_valid()
                && pte.flags().contains(PTEFlags::U)
                && if write {
                    pte.writable()
                } else {
                    pte.readable()
                }
        })
    })
}

/// Copy bytes into a fully validated user buffer, page by page.
pub fn copy_to_user(token: usize, start: usize, bytes: &[u8]) -> bool {
    if !user_buffer_valid(token, start, bytes.len(), true) {
        return false;
    }
    let mut offset = 0;
    for buffer in translated_byte_buffer(token, start as *const u8, bytes.len()) {
        let len = buffer.len();
        buffer.copy_from_slice(&bytes[offset..offset + len]);
        offset += len;
    }
    true
}

/// Read one byte only when the mapping is valid and user-readable.
pub fn read_user_byte(token: usize, addr: usize) -> Option<u8> {
    if !user_buffer_valid(token, addr, 1, false) {
        return None;
    }
    Some(translated_byte_buffer(token, addr as *const u8, 1)[0][0])
}
