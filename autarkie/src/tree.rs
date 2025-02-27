use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Debug,
    marker::PhantomData,
};

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
    fn __autarkie_generate(visitor: &mut Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self;

    fn __autarkie_register(v: &mut Visitor, parent: Option<Id>, variant: usize) {
        v.register_ty(parent, Self::__autarkie_id(), variant);
        v.pop_ty();
    }

    #[cfg(debug_assertions)]
    fn __autarkie_id() -> Id {
        std::intrinsics::type_name::<Self>().to_string()
    }

    #[cfg(not(debug_assertions))]
    fn __autarkie_id() -> Id {
        std::intrinsics::type_id::<Self>()
    }

    fn inner_id() -> Id {
        Self::__autarkie_id()
    }

    fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {}

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {}

    fn __autarkie_node_ty(&self) -> NodeType {
        NodeType::NonRecursive
    }

    fn __autarkie_serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
        Some(vec![(serialize(&self), Self::__autarkie_id())])
    }

    fn __autarkie_mutate(&mut self, ty: &mut MutationType, visitor: &mut Visitor, path: VecDeque<usize>) {
        debug_assert!(path.len() == 0);
        match ty {
            MutationType::Splice(other) => {
                *self = deserialize(other);
            }
            MutationType::GenerateReplace(ref mut bias) => {
                *self = Self::__autarkie_generate(visitor, bias, &mut 0);
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
node!(Debug + serde::ser::Serialize + for<'a> serde::de::Deserialize<'a> + 'static);

#[cfg(feature = "scale")]
node!(Debug + parity_scale_codec::Encode + parity_scale_codec::Decode + 'static);

#[cfg(feature = "borsh")]
node!(Debug + borsh::BorshSerialize + borsh::BorshDeserialize + 'static);

impl<T: 'static> Node for PhantomData<T> {
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: &mut usize,
    ) -> Self {
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
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: &mut usize,
    ) -> Self {
        // TODO: optimize?
        (0..N)
            .map(|_| T::__autarkie_generate(visitor, &mut visitor.generate_depth(), cur_depth))
            .collect::<Vec<T>>()
            .try_into()
            .expect("invariant;")
    }

    fn __autarkie_serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
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

    fn __autarkie_node_ty(&self) -> NodeType {
        NodeType::Iterable(true, N, T::__autarkie_id())
    }

    fn __autarkie_register(v: &mut Visitor, parent: Option<Id>, variant: usize) {
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
            visitor.register_field_stack(((index, child.__autarkie_node_ty()), T::__autarkie_id()));
            child.__autarkie_fields(visitor, 0);
            visitor.pop_field();
        }
    }

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        for (index, child) in self.iter().enumerate() {
            visitor
                .register_field_stack((((index, child.__autarkie_node_ty())), T::__autarkie_id()));
            child.__autarkie_cmps(visitor, index, val);
            visitor.pop_field();
        }
    }
}

impl<T> Node for Vec<T>
where
    T: Node + Debug,
{
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: &mut usize,
    ) -> Self {
        let element_count = if *depth > 0 {
            visitor.random_range(if *cur_depth == 0 { 1 } else { 0 }, visitor.iterate_depth())
        } else {
            50
        };
        if element_count == 0 {
            return vec![];
        }
        let mut vector = Vec::with_capacity(element_count);
        for i in 0..element_count {
            vector.push(T::__autarkie_generate(visitor, &mut 0, cur_depth));
        }
        vector
    }

    fn __autarkie_register(v: &mut Visitor, parent: Option<Id>, variant: usize) {
        if !v.is_recursive(T::__autarkie_id()) {
            T::__autarkie_register(v, parent, variant);
        } else {
            v.register_ty(parent, T::__autarkie_id(), variant);
            v.pop_ty();
        }
    }

    fn __autarkie_node_ty(&self) -> NodeType {
        NodeType::Iterable(false, self.len(), Self::inner_id())
    }

    fn inner_id() -> Id {
        T::__autarkie_id()
    }

    fn __autarkie_serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
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

    fn __autarkie_mutate(
        &mut self,
        ty: &mut MutationType,
        visitor: &mut Visitor,
        mut path: VecDeque<usize>,
    ) {
        if let Some(popped) = path.pop_front() {
            self.get_mut(popped)
                .expect("UbEi1VMg____")
                .__autarkie_mutate(ty, visitor, path);
        } else {
            match ty {
                MutationType::Splice(other) => {
                    *self = deserialize(other);
                }
                MutationType::GenerateReplace(ref mut bias) => {
                    *self = Self::__autarkie_generate(visitor, bias, &mut 0)
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

    fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {
        for (index, child) in self.iter().enumerate() {
            visitor.register_field_stack(((index, child.__autarkie_node_ty()), T::__autarkie_id()));
            child.__autarkie_fields(visitor, 0);
            visitor.pop_field();
        }
    }

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        for (index, child) in self.iter().enumerate() {
            visitor
                .register_field_stack((((index, child.__autarkie_node_ty())), T::__autarkie_id()));
            child.__autarkie_cmps(visitor, index, val);
            visitor.pop_field();
        }
    }
}

impl Node for bool {
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: &mut usize,
    ) -> Self {
        visitor.coinflip()
    }
}

impl<T> Node for Box<T>
where
    T: Node + Debug + Clone,
{
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: &mut usize,
    ) -> Self {
        Box::new(T::__autarkie_generate(visitor, depth, cur_depth))
    }

    fn inner_id() -> Id {
        T::__autarkie_id()
    }

    fn __autarkie_register(v: &mut Visitor, parent: Option<Id>, variant: usize) {
        if !v.is_recursive(T::__autarkie_id()) {
            T::__autarkie_register(v, parent, variant);
        } else {
            v.register_ty(parent, T::__autarkie_id(), variant);
            v.pop_ty();
        }
    }

    fn __autarkie_node_ty(&self) -> NodeType {
        self.as_ref().__autarkie_node_ty()
    }

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        self.as_ref().__autarkie_cmps(visitor, index, val);
    }

    fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {
        self.as_ref().__autarkie_fields(visitor, index);
    }

    fn __autarkie_mutate(
        &mut self,
        ty: &mut MutationType,
        visitor: &mut Visitor,
        path: VecDeque<usize>,
    ) {
        self.as_mut().__autarkie_mutate(ty, visitor, path);
    }

    fn __autarkie_serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
        self.as_ref().__autarkie_serialized()
    }
}

impl<T> Node for Option<T>
where
    T: Node + Debug,
{
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: &mut usize,
    ) -> Self {
        let choose_some = visitor.coinflip();
        if choose_some {
            Some(T::__autarkie_generate(visitor, depth, cur_depth))
        } else {
            None
        }
    }

    fn inner_id() -> Id {
        T::__autarkie_id()
    }
    // PhantomData<bool> is used as a dummy value for "None"
    fn __autarkie_register(v: &mut Visitor, parent: Option<Id>, variant: usize) {
        v.register_ty(parent, Self::__autarkie_id(), variant);
        if !v.is_recursive(T::__autarkie_id()) {
            T::__autarkie_register(v, Some(Self::__autarkie_id()), 0);
            v.register_ty(
                Some(Self::__autarkie_id()),
                PhantomData::<bool>::__autarkie_id(),
                1,
            );
            v.pop_ty();
        } else {
            v.register_ty(Some(Self::__autarkie_id()), T::__autarkie_id(), 0);
            v.pop_ty();
            v.register_ty(
                Some(Self::__autarkie_id()),
                PhantomData::<bool>::__autarkie_id(),
                1,
            );
            v.pop_ty();
        }
        v.pop_ty();
    }

    fn __autarkie_mutate(
        &mut self,
        ty: &mut MutationType,
        visitor: &mut Visitor,
        mut path: VecDeque<usize>,
    ) {
        let popped = path.pop_front();
        if popped.is_some() && !self.is_none() {
            self.as_mut().unwrap().__autarkie_mutate(ty, visitor, path);
        } else {
            match ty {
                MutationType::Splice(other) => {
                    *self = deserialize(other);
                }
                MutationType::GenerateReplace(ref mut bias) => {
                    *self = Self::__autarkie_generate(visitor, bias, &mut 0)
                }
                _ => {
                    unreachable!()
                }
            }
        }
    }

    // TODO: for now we perform duplicate serialization cause the inner field is also serialized.
    // and our parent will serialize us
    fn __autarkie_serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
        if let Some(inner) = self {
            let mut vector = vec![(serialize(&inner), T::__autarkie_id())];
            if let Some(inner_fields) = inner.__autarkie_serialized() {
                vector.extend(inner_fields)
            }
            Some(vector)
        } else {
            None
        }
    }

    fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {
        if let Some(inner) = self {
            visitor.register_field_stack(((index, inner.__autarkie_node_ty()), T::__autarkie_id()));
            inner.__autarkie_fields(visitor, 0);
            visitor.pop_field();
        }
    }

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        if let Some(inner) = self {
            visitor.register_field_stack(((index, inner.__autarkie_node_ty()), T::__autarkie_id()));
            inner.__autarkie_cmps(visitor, 0, val);
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
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: &mut usize,
    ) -> Self {
        let choose_ok = visitor.coinflip();
        if choose_ok {
            Ok(T::__autarkie_generate(visitor, depth, cur_depth))
        } else {
            Err(E::__autarkie_generate(visitor, depth, cur_depth))
        }
    }

    fn __autarkie_register(v: &mut Visitor, parent: Option<Id>, variant: usize) {
        v.register_ty(parent, Self::__autarkie_id(), variant);
        if !v.is_recursive(T::__autarkie_id()) {
            T::__autarkie_register(v, Some(Self::__autarkie_id()), 0);
        } else {
            v.register_ty(Some(Self::__autarkie_id()), T::__autarkie_id(), 0);
            v.pop_ty();
        }
        if !v.is_recursive(E::__autarkie_id()) {
            E::__autarkie_register(v, Some(Self::__autarkie_id()), 1);
        } else {
            v.register_ty(Some(Self::__autarkie_id()), E::__autarkie_id(), 1);
            v.pop_ty();
        }
        v.pop_ty();
    }
    fn __autarkie_mutate(
        &mut self,
        ty: &mut MutationType,
        visitor: &mut Visitor,
        mut path: VecDeque<usize>,
    ) {
        if let Some(popped) = path.pop_front() {
            if popped == 0 {
                if let Ok(ref mut inner) = self {
                    inner.__autarkie_mutate(ty, visitor, path);
                } else {
                    unreachable!("____TVKKYCUo");
                }
            } else if let Err(ref mut inner) = self {
                inner.__autarkie_mutate(ty, visitor, path);
            } else {
                unreachable!("____5DNOpzaC");
            }
        } else {
            match ty {
                MutationType::Splice(other) => {
                    *self = deserialize(other);
                }
                MutationType::GenerateReplace(ref mut bias) => {
                    *self = Self::__autarkie_generate(visitor, bias, &mut 0);
                }
                _ => {
                    unreachable!()
                }
            }
        }
    }

    fn __autarkie_serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
        if let Ok(inner) = self {
            let mut vector = vec![(serialize(&inner), T::__autarkie_id())];
            if let Some(inner_fields) = inner.__autarkie_serialized() {
                vector.extend(inner_fields)
            }
            Some(vector)
        } else if let Err(inner) = self {
            let mut vector = vec![(serialize(&inner), T::__autarkie_id())];
            if let Some(inner_fields) = inner.__autarkie_serialized() {
                vector.extend(inner_fields)
            }
            Some(vector)
        } else {
            unreachable!("zKJv3wsE____")
        }
    }

    fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {
        visitor.register_field_stack(((index, self.__autarkie_node_ty()), Self::__autarkie_id()));
        if let Ok(inner) = self {
            inner.__autarkie_fields(visitor, 0);
        } else if let Err(inner) = self {
            inner.__autarkie_fields(visitor, 1);
        }
        visitor.pop_field();
    }

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        visitor.register_field_stack(((index, self.__autarkie_node_ty()), Self::__autarkie_id()));
        if let Ok(inner) = self {
            inner.__autarkie_cmps(visitor, 0, val);
        } else if let Err(inner) = self {
            inner.__autarkie_cmps(visitor, 1, val);
        }
        visitor.pop_field();
    }
}

impl Node for std::string::String {
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: &mut usize,
    ) -> Self {
        visitor.get_string()
    }
}

impl Node for char {
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: &mut usize,
    ) -> Self {
        char::from_u32(u32::__autarkie_generate(visitor, depth, cur_depth)).unwrap_or_default()
    }
}

impl<K, V> Node for BTreeMap<K, V>
where
    K: Node + Clone + Ord,
    V: Node + Clone,
{
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: &mut usize,
    ) -> Self {
        BTreeMap::new()
    }

    fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {
        // TODO: does deferencing clone?
        for (index, (k, v)) in self.into_iter().enumerate() {
            visitor
                .register_field_stack(((index, NodeType::NonRecursive), <(K, V)>::__autarkie_id()));
            k.__autarkie_fields(visitor, 0);
            v.__autarkie_fields(visitor, 1);
            visitor.pop_field();
        }
    }

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        for (index, (k, v)) in self.into_iter().enumerate() {
            visitor
                .register_field_stack(((index, NodeType::NonRecursive), <(K, V)>::__autarkie_id()));
            k.__autarkie_cmps(visitor, 0, val);
            v.__autarkie_cmps(visitor, 1, val);
            visitor.pop_field();
        }
    }

    fn __autarkie_node_ty(&self) -> NodeType {
        NodeType::Iterable(false, self.len(), Self::__autarkie_id())
    }

    fn __autarkie_mutate(
        &mut self,
        ty: &mut MutationType,
        visitor: &mut Visitor,
        mut path: VecDeque<usize>,
    ) {
        if let Some(popped) = path.pop_front() {
            let mut entry_to_modify = None;
            for (i, (k, v)) in self.iter().enumerate() {
                if i == popped {
                    entry_to_modify = Some(k.clone());
                    break;
                }
            }
            let mut entry_to_modify = entry_to_modify.expect("XaLl1F31____");
            // we are mutating the (k, v) tuple
            if path.is_empty() {
                // let's remove the entry.
                self.remove(&entry_to_modify).expect("WDZstzcR____");
                match ty {
                    MutationType::Splice(other) => {
                        let (k, v) = deserialize(other);
                        self.insert(k, v);
                    }
                    MutationType::GenerateReplace(bias) => {
                        self.insert(
                            K::__autarkie_generate(visitor, bias, &mut 0),
                            V::__autarkie_generate(visitor, bias, &mut 0),
                        );
                    }
                    _ => unreachable!(),
                }
            } else {
                // We are mutating either the key or the value.
                let inner_popped = path.pop_front().expect("YQ8z8tF8____");
                // key == 0; value == 1
                debug_assert!(inner_popped == 0 || inner_popped == 1);
                if inner_popped == 0 {
                    let val = self.remove(&entry_to_modify).expect("WDZstzcR____");
                    entry_to_modify.__autarkie_mutate(ty, visitor, path);
                    self.insert(entry_to_modify, val);
                } else {
                    self.get_mut(&entry_to_modify)
                        .expect("yMhZ8dor____")
                        .__autarkie_mutate(ty, visitor, path);
                }
            }
        } else {
            match ty {
                MutationType::Splice(other) => {
                    *self = deserialize(other);
                }
                MutationType::GenerateReplace(ref mut bias) => {
                    *self = Self::__autarkie_generate(visitor, bias, &mut 0)
                }
                MutationType::SpliceAppend(other) => {
                    let (k, v) = deserialize(other);
                    self.insert(k, v);
                }
                MutationType::IterablePop(ref mut bias) => {
                    let mut remove_key = None;
                    for (i, (k, v)) in self.iter().enumerate() {
                        if i == *bias {
                            remove_key = Some(k.clone());
                            break;
                        }
                    }
                    self.remove(&remove_key.expect("2kejvSX9____"))
                        .expect("WDZstzcR____");
                }
                MutationType::RecursiveReplace => {
                    // TODO
                }
            }
        }
    }

    fn __autarkie_serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
        None
    }
}

macro_rules! tuple_impls {
    ( $( ($T:ident , $id:tt)),+ ) => {
        impl<$($T: Node),+> Node for ($($T,)+)
        {
            fn __autarkie_generate(
                visitor: &mut Visitor,
                depth: &mut usize, cur_depth: &mut usize
            ) -> Self {
                ($($T::__autarkie_generate(visitor, depth, cur_depth),)+)
            }
            fn __autarkie_mutate(&mut self, ty: &mut MutationType, visitor: &mut Visitor,  mut path: VecDeque<usize>) {
                if let Some(popped) = path.pop_front() {
                    match popped {
                        $($id => {
                            self.$id.__autarkie_mutate(ty, visitor, path)
                         }),*
                        _ => unreachable!("____okr3j4TT"),
                    }
                } else {
                    match ty {
                        MutationType::Splice(other) => {
                            *self = deserialize(other);
                        },
                        MutationType::GenerateReplace(ref mut bias) => {
                            *self = Self::__autarkie_generate(visitor, bias, &mut 0);
                        },
            _  => {
                unreachable!()
            }
                    }
                }
            }
            fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {
                $({
                visitor.register_field_stack(((($id, crate::NodeType::NonRecursive)), $T::__autarkie_id()));
                self.$id.__autarkie_fields(visitor, 0);
                visitor.pop_field();
                })*
            }

            fn __autarkie_register(v: &mut Visitor, parent: Option<Id>, variant: usize) {
                v.register_ty(parent, Self::__autarkie_id(), variant);
                $({
                if !v.is_recursive($T::__autarkie_id()) {
                    $T::__autarkie_register(v, Some(Self::__autarkie_id()), 0);
                } else {
                    v.register_ty(Some(Self::__autarkie_id()), $T::__autarkie_id(), 0);
                    v.pop_ty();
                }
                })*
                v.pop_ty();
            }

            fn __autarkie_serialized(&self) -> Option<Vec<(Vec<u8>, Id)>> {
                let mut vector = Vec::new();
                $(vector.push((serialize(&self.$id), $T::__autarkie_id()));)*
                $({
                    if let Some(inner) = self.$id.__autarkie_serialized() {
                        vector.extend(inner)
                    }
                })*
                    Some(vector)
            }

            fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
                $({
                visitor.register_field_stack(((($id, crate::NodeType::NonRecursive)), $T::__autarkie_id()));
                self.$id.__autarkie_cmps(visitor, 0, val);
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
            fn __autarkie_generate(
                v: &mut Visitor,
                depth: &mut usize,
                cur_depth: &mut usize,
            ) -> Self {
                deserialize::<Self>(&mut v.generate_bytes($num_bytes).as_slice())
            }
            fn __autarkie_cmps(&self, v: &mut Visitor, index: usize, val: (u64, u64)) {
                if val.0 == *self as u64 {
                    v.register_cmp(serialize(&(val.1 as Self)));
                }
            }
        }
    };
    // we don't do cmps for u8
    (u8, $num_bytes: literal) => {
        impl Node for $type {
            fn __autarkie_generate(v: &mut Visitor) -> Self {
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
