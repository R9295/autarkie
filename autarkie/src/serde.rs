use std::{collections::VecDeque, fmt::Debug};

use crate::{deserialize, serialize, MutationType, Node, Visitor};

macro_rules! impl_node_serde_array {
    ($n: literal) => {
        impl<T> Node for [T; $n]
        where
            // TODO can we remove the debug clause?
            T: Node + Debug,
        {
            fn __autarkie_generate(
                visitor: &mut Visitor,
                depth: &mut usize,
                cur_depth: &mut usize,
            ) -> Self {
                // TODO: optimize?
                (0..$n)
                    .map(|_| T::__autarkie_generate(visitor, depth, cur_depth))
                    .collect::<Vec<T>>()
                    .try_into()
                    .expect("invariant;")
            }

            fn __autarkie_serialized(&self) -> Option<Vec<(Vec<u8>, crate::Id)>> {
                let mut vector = self
                    .iter()
                    .map(|i| (serialize(i), T::__autarkie_id()))
                    .collect::<Vec<_>>();
                for item in self.iter() {
                    if let Some(inner) = item.__autarkie_serialized() {
                        vector.extend(inner)
                    }
                }
                Some(vector)
            }

            fn __autarkie_node_ty(&self) -> crate::NodeType {
                crate::NodeType::Iterable(true, $n, T::__autarkie_id())
            }

            fn __autarkie_register(v: &mut Visitor, parent: Option<crate::Id>, variant: usize) {
                if !v.is_recursive(T::__autarkie_id()) {
                    T::__autarkie_register(v, parent, variant);
                } else {
                    v.register_ty(parent, T::__autarkie_id(), variant);
                    v.pop_ty();
                }
            }

            fn __autarkie_mutate(
                &mut self,
                ty: &mut MutationType,
                visitor: &mut Visitor,
                mut path: VecDeque<usize>,
            ) {
                if let Some(popped) = path.pop_front() {
                    self.get_mut(popped)
                        .expect("mdNWnhI6____")
                        .__autarkie_mutate(ty, visitor, path);
                } else {
                    match ty {
                        MutationType::Splice(other) => {
                            *self = deserialize(other);
                        }
                        MutationType::GenerateReplace(ref mut bias) => {
                            *self = Self::__autarkie_generate(visitor, bias, &mut 0)
                        }
                        _ => unreachable!("tAL6LPUb____"),
                    }
                }
            }
            fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {
                for (index, child) in self.iter().enumerate() {
                    visitor.register_field_stack((
                        ((index, child.__autarkie_node_ty())),
                        T::__autarkie_id(),
                    ));
                    child.__autarkie_fields(visitor, 0);
                    visitor.pop_field();
                }
            }

            fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
                for (index, child) in self.iter().enumerate() {
                    visitor.register_field_stack((
                        ((index, child.__autarkie_node_ty())),
                        T::__autarkie_id(),
                    ));
                    child.__autarkie_cmps(visitor, index, val);
                    visitor.pop_field();
                }
            }
        }
    };
}

impl_node_serde_array!(1usize);
impl_node_serde_array!(2usize);
impl_node_serde_array!(3usize);
impl_node_serde_array!(4usize);
impl_node_serde_array!(5usize);
impl_node_serde_array!(6usize);
impl_node_serde_array!(7usize);
impl_node_serde_array!(8usize);
impl_node_serde_array!(9usize);
impl_node_serde_array!(10usize);
impl_node_serde_array!(11usize);
impl_node_serde_array!(12usize);
impl_node_serde_array!(13usize);
impl_node_serde_array!(14usize);
impl_node_serde_array!(15usize);
impl_node_serde_array!(16usize);
impl_node_serde_array!(17usize);
impl_node_serde_array!(18usize);
impl_node_serde_array!(19usize);
impl_node_serde_array!(20usize);
impl_node_serde_array!(21usize);
impl_node_serde_array!(22usize);
impl_node_serde_array!(23usize);
impl_node_serde_array!(24usize);
impl_node_serde_array!(25usize);
impl_node_serde_array!(26usize);
impl_node_serde_array!(27usize);
impl_node_serde_array!(28usize);
impl_node_serde_array!(29usize);
impl_node_serde_array!(30usize);
impl_node_serde_array!(31usize);
impl_node_serde_array!(32usize);
