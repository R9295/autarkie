/// Primitives for parity-scale-codec
use parity_scale_codec::{Compact, Decode, Encode};
use serde::{Deserialize, Serialize};
use crate::MutationType;
use std::collections::VecDeque;
use crate::Visitor;
use crate::Node;

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct Compactu16(#[codec(compact)] pub u16);

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct Compactu32(#[codec(compact)] pub u32);

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct Compactu64(#[codec(compact)] pub u64);

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct Compactu128(#[codec(compact)] pub u128);

macro_rules! impl_generate_compact {
    ($type: ty, $inner: ty, $num_bytes: literal) => {
        impl Node for $type {
            fn __autarkie_generate(
                v: &mut crate::Visitor,
                depth: &mut usize,
                cur_depth: usize,
                settings: Option<crate::GenerateSettings>
            ) -> Option<Self> {
                    let Some(compacted) = crate::maybe_deserialize::<Compact<$inner>>(&mut v.generate_bytes($num_bytes).as_slice()) else {
                        return None;
                    };
                    Some(Self(compacted.0))
                }
            fn __autarkie_cmps(&self, v: &mut crate::Visitor, index: usize, val: (u64, u64)) {
                if val.0 == self.0 as u64 {
                    v.register_cmp(crate::serialize(&(val.1 as $inner)));
                };
            }
            fn __autarkie_mutate(&mut self, ty: &mut MutationType, visitor: &mut Visitor, path: VecDeque<usize>) {
                debug_assert!(path.len() == 0);
                match ty {
                    MutationType::Splice(other) => {
                        // TODO: debug
                        if let Some(compacted) = crate::maybe_deserialize::<Compact<$inner>>(other) {
                            *self = Self(compacted.0);
                        }
                    }
                    MutationType::GenerateReplace(ref mut bias) => {
                        if let Some(generated) = Self::__autarkie_generate(visitor, bias, 0, None) {
                            *self = generated;
                            visitor.add_serialized(crate::serialize(self), Self::__autarkie_id());
                        }
                    }
                    _ => {
                        unreachable!()
                    }
                }
            }
        }
    };
}

impl_generate_compact!(Compactu16, u16, 2);
impl_generate_compact!(Compactu32, u32, 4);
impl_generate_compact!(Compactu64, u64, 8);
impl_generate_compact!(Compactu128, u128, 32);

