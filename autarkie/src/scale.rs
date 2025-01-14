use parity_scale_codec::{Compact, Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::Node;

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct CompactU8(#[codec(compact)] pub u8);

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct CompactU16(#[codec(compact)] pub u16);

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct CompactU32(#[codec(compact)] pub u32);

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct CompactU64(#[codec(compact)] pub u64);

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct CompactU128(#[codec(compact)] pub u128);

macro_rules! impl_generate_compact {
    ($type: ty, $inner: ty, $num_bytes: literal) => {
        impl Node for $type {
            fn generate(v: &mut crate::Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
                let inner = crate::deserialize::<$inner>(&mut v.generate_bytes($num_bytes).as_slice());
                Self(inner)
            }
            fn cmps(&self, v: &mut crate::Visitor, index: usize, val: (u64, u64)) {
                if val.0 == self.0 as u64 {
                    v.register_cmp(crate::serialize(&(val.1 as $inner)));
                };
            }
        }
    };
    // we don't do cmps for u8
    (u8, $num_bytes: literal) => {
        impl Node for $type {
            fn generate(v: &mut Visitor) -> Self {
                let inner = crate::deserialize::<$inner>(&mut v.generate_bytes($num_bytes).as_slice());
                Self(inner)
            }
        }
    };
}

impl_generate_compact!(CompactU8, u8, 1);
impl_generate_compact!(CompactU16, u16, 2);
impl_generate_compact!(CompactU32, u32, 4);
impl_generate_compact!(CompactU64, u64, 8);
impl_generate_compact!(CompactU128, u128, 32);
