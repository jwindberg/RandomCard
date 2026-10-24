//! HyperCard's stack password scheme, used for RandomCard's own stack
//! protection too, so imported passwords keep working.
//!
//! Ported from StackBlockDecrypter.swift in Pierre Lorenzi's HyperCardPreview
//! (MIT License, Copyright (c) 2018 Pierre Lorenzi):
//! <https://github.com/PierreLorenzi/HyperCardPreview>
//!
//! This only deters casual changes: the hash is weak, and stack files can be
//! edited directly.

/// Old Mac OS's Random function, which HyperCard used for hashing.
pub fn hash_number(x: u32) -> u32 {
    let mut r = (x as u64) * 0x41a7;
    r += r >> 31;
    (r & 0x7fff_ffff) as u32
}

/// Passwords ignore case and accents.
fn normalize(password: &str) -> Vec<u8> {
    password
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'a',
            'ç' | 'Ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
            'ñ' | 'Ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
            'ÿ' | 'Ÿ' => 'y',
            c => c.to_ascii_lowercase(),
        })
        .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
        .collect()
}

/// Hash a password's bytes the way HyperCard did.
pub fn hash_bytes(password: &[u8]) -> u32 {
    let first = password.first().copied().unwrap_or(0) as u32;
    let mut s = first + password.len() as u32;
    if s > 0xff {
        s &= 0xff;
    } else if first > 0x80 {
        s |= 0xffff_ff00;
    }
    let mut x: u32 = 0;
    for &c in password {
        for bit in 0..8 {
            s = hash_number(s);
            if (c >> (7 - bit)) & 1 != 0 {
                x = x.wrapping_add(s);
            }
        }
    }
    if x == 0 { 0x4269_6c6c } else { x } // 'Bill'
}

/// The first hash, which also keys the encryption of private-access stacks.
pub fn first_hash(password: &str) -> u32 {
    hash_bytes(&normalize(password))
}

/// The hash a stack stores: the first hash, hashed again as a 4-byte string.
pub fn stored_hash(password: &str) -> u32 {
    hash_bytes(&first_hash(password).to_be_bytes())
}

pub fn check(password: &str, stored: u32) -> bool {
    stored_hash(password) == stored
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hypercard_password_hash() {
        // From HyperCardPreview's TestPassword stack.
        assert_eq!(stored_hash("AA éé 1234 ÀÂä"), 0xCA92_2FEB);
        assert!(check("aa ee 1234 aaa", 0xCA92_2FEB));
        assert!(!check("coucou", 0xCA92_2FEB));
    }
}
