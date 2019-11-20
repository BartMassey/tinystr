use std::cmp::Ordering;
use std::fmt;
use std::ops::Deref;
use std::ptr::copy_nonoverlapping;
use std::str::FromStr;
use std::marker::PhantomData;

use crate::Error;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct TinyStr<T, NZT> {
    ts: NZT,
    base: PhantomData<T>,
}

trait GenMask {
    fn genmask(byte: u8) -> Self;
}

impl GenMask for u32 {
    fn genmask(byte: u8) -> u32 {
        let mask: u32 = byte.into();
        mask<<24|mask<<16|mask<<8|mask
    }
}

impl GenMask for u64 {
    fn genmask(byte: u8) -> u64 {
        let mask: u32 = u32::genmask(byte);
        let mask: u64 = mask.into();
        mask << 32|mask
    }
}

impl GenMask for u128 {
    fn genmask(byte: u8) -> u128 {
        let mask: u64 = u64::genmask(byte);
        let mask: u128 = mask.into();
        mask << 64 | mask
    }
}

#[test]
fn test_genmask() {
    assert_eq!(0xf0f0f0f0u32, u32::genmask(0xf0));
    assert_eq!(0xf0f0f0f0_f0f0f0f0u64, u64::genmask(0xf0));
    assert_eq!(0xf0f0f0f0_f0f0f0f0_f0f0f0f0_f0f0f0f0u128, u128::genmask(0xf0));
}

trait Zt<T> {
    fn zt(self) -> T;
}

impl<T, NZT> Zt<T> for TinyStr<T, NZT> {
    fn zt(self) -> T {
        self.zt.get::<NZT>()
    }
}

pub trait TinyType<T: Copy + GenMask + Zt>: Deref<&str> {
    fn size() -> usize {
        std::mem::size_of::<T>()
    }

    pub unsafe fn new_unchecked(text: T) -> Self;

    pub fn as_str(&self) -> &str {
        self.deref()
    }

    pub fn to_ascii_uppercase(self) -> Self {
        let mask_1f: T = T::genmask(0x1f);
        let mask_05: T = T::genmask(0x05);
        let mask_80: T = T::genmask(0x80);
        let word = self.zt();
        let result =
            word
            & !(((word + MASK_1F)
                 & !(word + MASK_05)
                 & MASK_80)
                >> 2);
        unsafe { Self::new_unchecked(result) }
    }

    pub fn to_ascii_lowercase(self) -> Self {
        let mask_3f: T = T::genmask(0x3f);
        let mask_25: T = T::genmask(0x25);
        let mask_80: T = T::genmask(0x80);
        let word = self.zt();
        let result = word
            | (((word + mask_3f)
                & !(word + mask_25)
                & mask_80)
               >> 2);
        unsafe { Self::new_unchecked(result) }
    }

    /// Makes the string all lowercase except for
    /// the first character, which is made
    /// uppercase.
    pub fn to_ascii_titlecase(self) -> $ty {
        const mask_1f: T = T::genmask(0x3f) & !0x20;
        const mask_05: T = T::genmask(0x25) & !0x20;
        const mask_80: T = T::genmask(0x80);
        let word = self.zt();
        let mask = ((word + mask_1F) & !(word + mask_05) & mask_80) >> 2;
        let result = (word | mask) & !(0x20 & mask);
        unsafe { Self::new_unchecked(result) }
    }

    pub fn is_ascii_alphanumeric(self) -> bool {
        const mask_7f: T = T::genmask(0x7f);
        const mask_80: T = T::genmask(0x80);
        const mask_20: T = T::genmask(0x20);
        const mask_1f: T = T::genmask(0x1f);
        const mask_05: T = T::genmask(0x05);
        let word = self.zt();
        let mask = (word + mask_7f) & mask_80;
        let lower = word | mask_20;
        ((!(lower + mask_1f) | (lower + mask_05)) & mask) == 0
    }
}

impl Deref for $ty {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &str {
        // Again, could use #cfg to hand-roll a big-endian implementation.
        let word = self.0.get().to_le();
        let len = <$ty>::size() - word.leading_zeros() as usize / 8;
        unsafe {
            let slice = core::slice::from_raw_parts(&self.0.get() as *const _ as *const u8, len);
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
        self.0.get().to_be().cmp(&other.0.get().to_be())
    }
}

impl From<$ty> for $ut {
    fn from(val: $ty) -> $ut {
        val.0.get().to_le()
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
    ($ty:ty, $nzt:ty, $ut:ty, $gm:ident) => {
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
                    Ok(Self(<$nzt>::new_unchecked(word)))
                }
            }
        }
    };
}

impl_from_str!(TinyStr8, NonZeroU64, u64, genmask8);
impl_from_str!(TinyStr16, NonZeroU128, u128, genmask16);
*/

        pub const unsafe fn new_unchecked(text: T) -> Self {
        Self(<$nzt>::new_unchecked(<$ut>::from_le(text)))
    }
