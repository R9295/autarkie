use std::{collections::VecDeque, fmt::Debug, marker::PhantomData};

use crate::{NodeType, Visitor};

#[cfg(debug_assertions)]
pub type Id = std::string::String;

#[cfg(not(debug_assertions))]
pub type Id = u128;

#[derive(Debug)]
pub enum MutationType<'a> {
    GenerateReplace(usize),
    IterablePop(usize),
    RecursiveReplace,
    Splice(&'a mut &'a [u8]),
    SpliceAppend(&'a mut &'a [u8]),
}

macro_rules! node {
    ($($bound:tt)*) => {
pub trait Node
where
    Self: $($bound)*
{
    /// Generate Self
    fn generate(visitor: &mut Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self;

    #[cfg(debug_assertions)]
    fn id() -> Id {
        std::intrinsics::type_name::<Self>().to_string()
    }

    #[cfg(not(debug_assertions))]
    fn id() -> Id {
        std::intrinsics::type_id::<Self>()
    }

    fn inner_id() -> Option<Id> {
        None
    }

    fn fields(&self, visitor: &mut Visitor, index: usize) {}

    fn cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {}

    fn node_ty(&self) -> NodeType {
        NodeType::NonRecursive
    }

    fn serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
        Some(vec![(serialize(&self), Self::id())])
    }

    fn __mutate(&mut self, ty: &mut MutationType, visitor: &mut Visitor, path: VecDeque<usize>) {
        debug_assert!(path.len() == 0);
        match ty {
            MutationType::Splice(other) => {
                *self = deserialize(other);
            }
            MutationType::GenerateReplace(ref mut bias) => {
                *self = Self::generate(visitor, bias, &mut 0);
            }
            _ => {
                unreachable!()
            }
        }
    }

}

    };
}

#[cfg(feature = "bincode")]
node!(Debug + serde::Serialize + for<'a> serde::Deserialize<'a> + 'static);

#[cfg(feature = "scale")]
node!(Debug + parity_scale_codec::Encode + parity_scale_codec::Decode + 'static);

#[cfg(feature = "borsh")]
node!(Debug + borsh::BorshSerialize + borsh::BorshDeserialize + 'static);

impl<T: 'static> Node for PhantomData<T> {
    fn generate(visitor: &mut Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
        Self
    }
}

// TODO: fix and make the same as Vec
#[cfg(any(feature = "borsh", feature = "scale"))]
impl<T, const N: usize> Node for [T; N]
where
    // TODO can we remove the debug clause?
    T: Node + Debug,
{
    fn generate(visitor: &mut Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
        // TODO: optimize?
        (0..N)
            .map(|_| T::generate(visitor, &mut visitor.generate_depth(), cur_depth))
            .collect::<Vec<T>>()
            .try_into()
            .expect("invariant;")
    }

    fn serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
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

    fn node_ty(&self) -> NodeType {
        NodeType::Iterable(true, N, T::id())
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
            visitor.register_field_stack(((index, child.node_ty()), T::id()));
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

impl<T> Node for Vec<T>
where
    T: Node + Debug,
{
    fn generate(visitor: &mut Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
        let element_count = if *depth > 0 {
            visitor.random_range(if *cur_depth == 0 { 1 } else { 0 }, visitor.iterate_depth())
        } else {
            0
        };
        if element_count == 0 {
            return vec![];
        }
        let mut vector = Vec::with_capacity(element_count);
        for i in 0..element_count {
            vector.push(T::generate(visitor, &mut 0, cur_depth));
        }
        vector
    }

    fn node_ty(&self) -> NodeType {
        NodeType::Iterable(false, self.len(), T::id())
    }

    fn serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
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

    fn __mutate(
        &mut self,
        ty: &mut MutationType,
        visitor: &mut Visitor,
        mut path: VecDeque<usize>,
    ) {
        if let Some(popped) = path.pop_front() {
            self.get_mut(popped)
                .expect("UbEi1VMg____")
                .__mutate(ty, visitor, path);
        } else {
            match ty {
                MutationType::Splice(other) => {
                    *self = deserialize(other);
                }
                MutationType::GenerateReplace(ref mut bias) => {
                    *self = Self::generate(visitor, bias, &mut 0)
                }
                MutationType::SpliceAppend(other) => {
                    self.push(deserialize(other));
                }
                MutationType::IterablePop(ref mut bias) => {
                    self.remove(*bias);
                }
                MutationType::RecursiveReplace => {
                    // TODO
                }
            }
        }
    }

    fn fields(&self, visitor: &mut Visitor, index: usize) {
        for (index, child) in self.iter().enumerate() {
            visitor.register_field_stack(((index, child.node_ty()), T::id()));
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

impl Node for bool {
    fn generate(visitor: &mut Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
        visitor.coinflip()
    }
}

impl<T> Node for Box<T>
where
    T: Node + Debug + Clone,
{
    fn generate(visitor: &mut Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
        Box::new(T::generate(visitor, depth, cur_depth))
    }

    fn inner_id() -> Option<Id> {
        Some(T::id())
    }

    fn node_ty(&self) -> NodeType {
        self.as_ref().node_ty()
    }

    fn cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        self.as_ref().cmps(visitor, index, val);
    }

    fn fields(&self, visitor: &mut Visitor, index: usize) {
        self.as_ref().fields(visitor, index);
    }

    fn __mutate(&mut self, ty: &mut MutationType, visitor: &mut Visitor, path: VecDeque<usize>) {
        self.as_mut().__mutate(ty, visitor, path);
    }

    fn serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
        self.as_ref().serialized()
    }
}

impl<T> Node for Option<T>
where
    T: Node + Debug,
{
    fn generate(visitor: &mut Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
        let choose_some = visitor.coinflip();
        if choose_some {
            Some(T::generate(visitor, depth, cur_depth))
        } else {
            None
        }
    }
    fn __mutate(
        &mut self,
        ty: &mut MutationType,
        visitor: &mut Visitor,
        mut path: VecDeque<usize>,
    ) {
        let popped = path.pop_front();
        if popped.is_some() && !self.is_none() {
            self.as_mut().unwrap().__mutate(ty, visitor, path);
        } else {
            match ty {
                MutationType::Splice(other) => {
                    *self = deserialize(other);
                }
                MutationType::GenerateReplace(ref mut bias) => {
                    *self = Self::generate(visitor, bias, &mut 0)
                }
                _ => {
                    unreachable!()
                }
            }
        }
    }

    // TODO: for now we perform duplicate serialization cause the inner field is also serialized.
    // and our parent will serialize us
    fn serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
        if let Some(inner) = self {
            let mut vector = vec![(serialize(&inner), T::id())];
            if let Some(inner_fields) = inner.serialized() {
                vector.extend(inner_fields)
            }
            Some(vector)
        } else {
            None
        }
    }

    fn fields(&self, visitor: &mut Visitor, index: usize) {
        if let Some(inner) = self {
            visitor.register_field_stack(((index, inner.node_ty()), T::id()));
            inner.fields(visitor, 0);
            visitor.pop_field();
        }
    }

    fn cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        if let Some(inner) = self {
            visitor.register_field_stack(((index, inner.node_ty()), T::id()));
            inner.cmps(visitor, 0, val);
            visitor.pop_field();
        }
    }
}

// This is very similar to the derive implementation fr Enum,
// When things get fucked -> just look at this to save yourself from macro hell
impl<T, E> Node for Result<T, E>
where
    T: Node + Debug,
    E: Node + Debug,
{
    fn generate(visitor: &mut Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
        let choose_ok = visitor.coinflip();
        if choose_ok {
            Ok(T::generate(visitor, depth, cur_depth))
        } else {
            Err(E::generate(visitor, depth, cur_depth))
        }
    }

    fn __mutate(
        &mut self,
        ty: &mut MutationType,
        visitor: &mut Visitor,
        mut path: VecDeque<usize>,
    ) {
        if let Some(popped) = path.pop_front() {
            if popped == 0 {
                if let Ok(ref mut inner) = self {
                    inner.__mutate(ty, visitor, path);
                } else {
                    unreachable!("____TVKKYCUo");
                }
            } else if let Err(ref mut inner) = self {
                inner.__mutate(ty, visitor, path);
            } else {
                unreachable!("____5DNOpzaC");
            }
        } else {
            match ty {
                MutationType::Splice(other) => {
                    *self = deserialize(other);
                }
                MutationType::GenerateReplace(ref mut bias) => {
                    *self = Self::generate(visitor, bias, &mut 0);
                }
                _ => {
                    unreachable!()
                }
            }
        }
    }

    fn serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
        if let Ok(inner) = self {
            let mut vector = vec![(serialize(&inner), T::id())];
            if let Some(inner_fields) = inner.serialized() {
                vector.extend(inner_fields)
            }
            Some(vector)
        } else if let Err(inner) = self {
            let mut vector = vec![(serialize(&inner), T::id())];
            if let Some(inner_fields) = inner.serialized() {
                vector.extend(inner_fields)
            }
            Some(vector)
        } else {
            unreachable!("zKJv3wsE____")
        }
    }

    fn fields(&self, visitor: &mut Visitor, index: usize) {
        visitor.register_field_stack(((index, self.node_ty()), Self::id()));
        if let Ok(inner) = self {
            inner.fields(visitor, 0);
        } else if let Err(inner) = self {
            inner.fields(visitor, 1);
        }
        visitor.pop_field();
    }

    fn cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        visitor.register_field_stack(((index, self.node_ty()), Self::id()));
        if let Ok(inner) = self {
            inner.cmps(visitor, 0, val);
        } else if let Err(inner) = self {
            inner.cmps(visitor, 1, val);
        }
        visitor.pop_field();
    }
}

impl Node for std::string::String {
    fn generate(visitor: &mut Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
        visitor.get_string()
    }
}

macro_rules! tuple_impls {
    ( $( ($T:ident , $id:tt)),+ ) => {
        impl<$($T: Node),+> Node for ($($T,)+)
        {
            fn generate(
                visitor: &mut Visitor,
                depth: &mut usize, cur_depth: &mut usize
            ) -> Self {
                ($($T::generate(visitor, depth, cur_depth),)+)
            }
            fn __mutate(&mut self, ty: &mut MutationType, visitor: &mut Visitor,  mut path: VecDeque<usize>) {
                if let Some(popped) = path.pop_front() {
                    match popped {
                        $($id => {
                            self.$id.__mutate(ty, visitor, path)
                         }),*
                        _ => unreachable!("____okr3j4TT"),
                    }
                } else {
                    match ty {
                        MutationType::Splice(other) => {
                            *self = deserialize(other);
                        },
                        MutationType::GenerateReplace(ref mut bias) => {
                            *self = Self::generate(visitor, bias, &mut 0);
                        },
            _  => {
                unreachable!()
            }
                    }
                }
            }
            fn fields(&self, visitor: &mut Visitor, index: usize) {
                $({
                visitor.register_field_stack(((($id, crate::NodeType::NonRecursive)), $T::id()));
                self.$id.fields(visitor, 0);
                visitor.pop_field();
                })*
            }
            fn serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
                let mut vector = Vec::new();
                $(vector.push((serialize(&self.$id), $T::id()));)*
                $({
                    if let Some(inner) = self.$id.serialized() {
                        vector.extend(inner)
                    }
                })*
                    Some(vector)
            }

            fn cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
                $({
                visitor.register_field_stack(((($id, crate::NodeType::NonRecursive)), $T::id()));
                self.$id.cmps(visitor, 0, val);
                visitor.pop_field();
                })*
            }
        }
    };
}

// some sort of Deity, please forgive me
tuple_impls! { (A,  0) }
tuple_impls! { (A , 0) ,(B, 1) }
tuple_impls! { (A , 0) ,(B, 1), (C, 2) }
tuple_impls! { (A , 0) ,(B, 1), (C, 2) ,(D, 3) }
tuple_impls! { (A , 0) ,(B, 1), (C, 2) ,(D, 3) ,(E, 4) }
tuple_impls! { (A , 0) ,(B, 1), (C, 2) ,(D, 3) ,(E, 4) ,(F, 5) }
tuple_impls! { (A , 0) ,(B, 1), (C, 2) ,(D, 3) ,(E, 4) ,(F, 5) ,(G, 6) }
tuple_impls! { (A , 0) ,(B, 1), (C, 2) ,(D, 3) ,(E, 4) ,(F, 5) ,(G, 6) ,(H, 7) }
tuple_impls! { (A , 0) ,(B, 1), (C, 2) ,(D, 3) ,(E, 4) ,(F, 5) ,(G, 6) ,(H, 7) ,(I, 8) }
tuple_impls! { (A , 0) ,(B, 1), (C, 2) ,(D, 3) ,(E, 4) ,(F, 5) ,(G, 6) ,(H, 7) ,(I, 8) ,(J, 9) }
tuple_impls! { (A , 0) ,(B, 1), (C, 2) ,(D, 3) ,(E, 4) ,(F, 5) ,(G, 6) ,(H, 7) ,(I, 8) ,(J, 9), (K, 10)}
tuple_impls! { (A , 0) ,(B, 1), (C, 2) ,(D, 3) ,(E, 4) ,(F, 5) ,(G, 6) ,(H, 7) ,(I, 8) ,(J, 9) ,(K , 10) ,(L, 11)}

macro_rules! impl_generate_simple {
    ($type: ty, $num_bytes: literal) => {
        impl Node for $type {
            fn generate(v: &mut Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
                deserialize::<Self>(&mut v.generate_bytes($num_bytes).as_slice())
            }
            fn cmps(&self, v: &mut Visitor, index: usize, val: (u64, u64)) {
                if val.0 == *self as u64 {
                    v.register_cmp(serialize(&(val.1 as Self)));
                };
            }
        }
    };
    // we don't do cmps for u8
    (u8, $num_bytes: literal) => {
        impl Node for $type {
            fn generate(v: &mut Visitor) -> Self {
                deserialize::<Self>(&mut v.generate_bytes($num_bytes).as_slice())
            }
        }
    };
}

impl_generate_simple!(f32, 4);
impl_generate_simple!(f64, 8);

impl_generate_simple!(u8, 1);
impl_generate_simple!(u16, 2);
impl_generate_simple!(u32, 4);
impl_generate_simple!(u64, 8);
impl_generate_simple!(u128, 32);
impl_generate_simple!(i8, 1);
impl_generate_simple!(i16, 2);
impl_generate_simple!(i32, 4);
impl_generate_simple!(i64, 8);
impl_generate_simple!(i128, 32);
#[cfg(feature = "bincode")]
impl_generate_simple!(isize, 8);
#[cfg(feature = "bincode")]
impl_generate_simple!(usize, 8);

#[cfg(feature = "bincode")]
pub fn serialize<T>(data: &T) -> Vec<u8>
where
    T: serde::Serialize,
{
    bincode::serialize(data).expect("invariant; we must always be able to serialize")
}

#[cfg(feature = "bincode")]
pub fn deserialize<T>(data: &mut &[u8]) -> T
where
    for<'a> T: serde::Deserialize<'a>,
{
    bincode::deserialize(data).expect("invariant; we must always be able to deserialize")
}

#[cfg(feature = "bincode")]
pub fn serialize_vec_len(len: usize) -> Vec<u8> {
    bincode::serialize(&(len as u64)).expect("invariant; we must always be able to serialize")
}

#[cfg(feature = "scale")]
pub fn serialize<T>(data: &T) -> Vec<u8>
where
    T: parity_scale_codec::Encode,
{
    T::encode(data)
}

#[cfg(feature = "scale")]
pub fn deserialize<T>(data: &mut &[u8]) -> T
where
    T: parity_scale_codec::Decode,
{
    let decoded = T::decode(data);
    if decoded.is_err() {
        println!("{:?}", std::intrinsics::type_name::<T>().to_string());
        println!("{:?}", data);
    }
    decoded.expect("invariant; we must always be able to deserialize")
}

#[cfg(feature = "scale")]
pub fn serialize_vec_len(len: usize) -> Vec<u8> {
    use parity_scale_codec::Encode;
    (parity_scale_codec::Compact(len as u32)).encode()
}

#[cfg(feature = "borsh")]
pub fn serialize<T>(data: &T) -> Vec<u8>
where
    T: borsh::BorshSerialize,
{
    borsh::to_vec(data).expect("invariant; we must always be able to deserialize")
}

#[cfg(feature = "borsh")]
pub fn deserialize<T>(data: &mut &[u8]) -> T
where
    T: borsh::BorshDeserialize,
{
    T::deserialize(data).expect("invariant; we must always be able to deserialize")
}

#[cfg(feature = "borsh")]
pub fn serialize_vec_len(len: usize) -> Vec<u8> {
    borsh::to_vec(&(len as u32)).expect("invariant; we must always be able to serialize")
}
