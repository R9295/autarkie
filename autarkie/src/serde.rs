use std::{collections::VecDeque, fmt::Debug};

use crate::{deserialize, serialize, MutationType, Node, Visitor};

macro_rules! impl_node_serde_array {
    ($n: literal) => {
        impl<T> Node for [T; $n]
        where
            // TODO can we remove the debug clause?
            T: Node + Debug,
        {
            fn generate(visitor: &mut Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
                // TODO: optimize?
                (0..$n)
                    .map(|_| T::generate(visitor, depth, cur_depth))
                    .collect::<Vec<T>>()
                    .try_into()
                    .expect("invariant;")
            }

            fn serialized(&self) -> Option<Vec<(Vec<u8>, crate::Id)>> {
                let mut vector = self
                    .iter()
                    .map(|i| (serialize(i), T::id()))
                    .collect::<Vec<_>>();
                for item in self.iter() {
                    if let Some(inner) = item.serialized() {
                        vector.extend(inner)
                    }
                }
                Some(vector)
            }

            fn node_ty(&self) -> crate::NodeType {
                crate::NodeType::Iterable(true, T::id())
            }

            fn __mutate(
                &mut self,
                ty: &mut MutationType,
                visitor: &mut Visitor,
                mut path: VecDeque<usize>,
            ) {
                if let Some(popped) = path.pop_front() {
                    self.get_mut(popped)
                        .expect("mdNWnhI6____")
                        .__mutate(ty, visitor, path);
                } else {
                    match ty {
                        MutationType::Splice(other) => {
                            *self = deserialize(other);
                        }
                        MutationType::GenerateReplace(ref mut bias) => {
                            *self = Self::generate(visitor, bias, &mut 0)
                        }
                        _ => unreachable!("tAL6LPUb____"),
                    }
                }
            }
            fn fields(&self, visitor: &mut Visitor, index: usize) {
                for (index, child) in self.iter().enumerate() {
                    visitor.register_field_stack((((index, child.node_ty())), T::id()));
                    child.fields(visitor, 0);
                    visitor.pop_field();
                }
            }

            fn cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
                for (index, child) in self.iter().enumerate() {
                    visitor.register_field_stack((((index, child.node_ty())), T::id()));
                    child.cmps(visitor, index, val);
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
