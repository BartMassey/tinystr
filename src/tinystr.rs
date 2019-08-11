use std::cmp::Ordering;
use std::fmt;
use std::ops::Deref;
use std::ptr::copy_nonoverlapping;
use std::str::FromStr;

use crate::Error;

#[inline(always)]
const fn genmask4(byte: u8) -> u32 {
    (byte as u32)<<24|(byte as u32)<<16|(byte as u32)<<8|(byte as u32)
}

#[inline(always)]
const fn genmask8(byte: u8) -> u64 {
    (genmask4(byte) as u64) << 32|(genmask4(byte) as u64)
}

#[inline(always)]
const fn genmask16(byte: u8) -> u128 {
    (genmask8(byte) as u128) << 64 | (genmask8(byte) as u128)
}

#[test]
fn test_genmask() {
    assert_eq!(0xf0f0f0f0u32, genmask4(0xf0));
    assert_eq!(0xf0f0f0f0_f0f0f0f0u64, genmask8(0xf0));
    assert_eq!(0xf0f0f0f0_f0f0f0f0_f0f0f0f0_f0f0f0f0u128, genmask16(0xf0));
}

macro_rules! tinytype {
    ($ty:ident, $nzt:ty, $ut:ty, $gm:ident) => {

        #[derive(Copy, Clone, PartialEq, Eq, Hash)]
        pub struct $ty($nzt);
        
        impl $ty {

            #[inline(always)]
            const fn get(&self) -> $ut {
                self.0.get()
            }

            #[inline(always)]
            const fn size() -> u32 {
                std::mem::size_of::<$ut>() as u32
            }

            #[inline(always)]
            pub unsafe fn new_unchecked(text: $ut) -> Self {
                $ty(<$nzt>::new_unchecked(<$ut>::from_le(text)))
            }

            #[inline(always)]
            pub fn as_str(&self) -> &str {
                self.deref()
            }

            pub fn to_ascii_uppercase(self) -> Self {
                let word = self.get();
                let result = word
                    & !(((word + $gm(0x1f))
                        & !(word + $gm(0x05))
                        & $gm(0x80))
                        >> 2);
                unsafe { Self::new_unchecked(result) }
            }

            pub fn to_ascii_lowercase(self) -> Self {
                let word = self.get();
                let result = word
                    | (((word + $gm(0x3f))
                        & !(word + $gm(0x25))
                        & $gm(0x80))
                        >> 2);
                unsafe { Self::new_unchecked(result) }
            }

            /// Makes the string all lowercase except for
            /// the first character, which is made
            /// uppercase.
            pub fn to_ascii_titlecase(self) -> $ty {
                let word = self.0.get().to_le();
                let mask = ((word + 0x3f3f_3f1f) & !(word + 0x2525_2505) & 0x8080_8080) >> 2;
                let result = (word | mask) & !(0x20 & mask);
                unsafe { $ty::new_unchecked(<$ut>::from_le(result)) }
            }

            pub fn is_ascii_alphanumeric(self) -> bool {
                let word = self.get();
                let mask = (word + $gm(0x7f)) & $gm(0x80);
                let lower = word | $gm(0x20);
                ((!(lower + $gm(0x1f)) | (lower + $gm(0x05))) & mask) == 0
            }

        }

        impl Deref for $ty {
            type Target = str;

            #[inline(always)]
            fn deref(&self) -> &str {
                // Again, could use #cfg to hand-roll a big-endian implementation.
                let word = self.get().to_le();
                let len = (Self::size() - word.leading_zeros() / 8) as usize;
                unsafe {
                    let slice = core::slice::from_raw_parts(&self.get() as *const _ as *const u8, len);
                    std::str::from_utf8_unchecked(slice)
                }
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}", self.deref())
            }
        }

        impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{:?}", self.deref())
            }
        }


        impl PartialEq<&str> for $ty {
            fn eq(&self, other: &&str) -> bool {
                self.deref() == *other
            }
        }

        impl PartialOrd for $ty {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $ty {
            fn cmp(&self, other: &Self) -> Ordering {
                self.get().to_be().cmp(&other.get().to_be())
            }
        }

        impl From<$ty> for $ut {
            fn from(val: $ty) -> $ut {
                val.get().to_le()
            }
        }

    }
}

use std::num::{NonZeroU32, NonZeroU64, NonZeroU128};

tinytype!(TinyStr4, NonZeroU32, u32, genmask4);
tinytype!(TinyStr8, NonZeroU64, u64, genmask8);
tinytype!(TinyStr16, NonZeroU128, u128, genmask16);

#[inline(always)]
unsafe fn make_4byte_str(
    text: &str,
    len: usize,
    mask: u32,
) -> Result<NonZeroU32, Error> {
    // Mask is always supplied as little-endian.
    let mask = u32::from_le(mask);
    let mut word: u32 = 0;
    copy_nonoverlapping(text.as_ptr(), &mut word as *mut u32 as *mut u8, len);
    if (word & mask) != 0 {
        return Err(Error::NonAscii);
    }
    if ((mask - word) & mask) != 0 {
        return Err(Error::InvalidNull);
    }
    Ok(NonZeroU32::new_unchecked(word))
}

impl FromStr for TinyStr4 {
    type Err = Error;

    #[inline(always)]
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        unsafe {
            match text.len() {
                1 => make_4byte_str(text, 1, 0x80).map(Self),
                2 => make_4byte_str(text, 2, 0x8080).map(Self),
                3 => make_4byte_str(text, 3, 0x0080_8080).map(Self),
                4 => make_4byte_str(text, 4, 0x8080_8080).map(Self),
                _ => Err(Error::InvalidSize),
            }
        }
    }
}

macro_rules! impl_from_str {
    ($ty:ty, $ut:ty, $gm:ident) => {
        impl FromStr for $ty {
            type Err = Error;

            #[inline(always)]
            fn from_str(text: &str) -> Result<Self, Self::Err> {
                let len = text.len();
                if len < 1 || len > Self::size() as usize {
                    return Err(Error::InvalidSize);
                }
                let mut word: $ut = 0;
                unsafe {
                    copy_nonoverlapping(
                        text.as_ptr(),
                        &mut word as *mut $ut as *mut u8,
                        len,
                    );
                }
                let mask = $gm(0x80) >> (8 * (Self::size() as usize - len));
                // TODO: could do this with
                // #cfg(target_endian), but this is clearer
                // and more confidence-inspiring.
                let mask = <$ut>::from_le(mask);
                if (word & mask) != 0 {
                    return Err(Error::NonAscii);
                }
                if ((mask - word) & mask) != 0 {
                    return Err(Error::InvalidNull);
                }
                unsafe {
                    Ok(Self::new_unchecked(word))
                }
            }
        }
    };
}

impl_from_str!(TinyStr8, u64, genmask8);
impl_from_str!(TinyStr16, u128, genmask16);
