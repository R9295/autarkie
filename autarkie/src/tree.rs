use crate::{visitor, NodeType, Visitor};
use serde::de::DeserializeOwned;
use std::any::TypeId;
use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::{
    collections::{BTreeMap, VecDeque},
    marker::PhantomData,
};

pub type Id = u64;

#[derive(Debug)]
pub enum MutationType<'a> {
    GenerateReplace(usize),
    IterablePop(usize),
    RecursiveReplace,
    Splice(&'a mut &'a [u8]),
    GenerateAppend(usize),
    SpliceAppend(&'a mut &'a [u8]),
}

#[derive(Debug)]
pub enum GenerateSettings {
    Length(usize),
    Range(std::ops::RangeInclusive<usize>),
}

macro_rules! node {
    ($($bound:tt)*) => {
pub trait Node
where
    Self: $($bound)*
{
    /// Generate Self
    fn __autarkie_generate(visitor: &mut Visitor, depth: &mut usize, cur_depth : usize, settings: Option<GenerateSettings>) -> Option<Self>;

    fn __autarkie_register(v: &mut Visitor, parent: Option<(Id, String)>, variant: usize) {
        v.register_ty(parent, Self::__autarkie_id_tuple(), variant);
        v.pop_ty();
    }


    fn __autarkie_id() -> Id {
        let mut hasher = twox_hash::XxHash64::default();
        let type_id = std::any::TypeId::of::<Self>();
        type_id.hash(&mut hasher);
        hasher.finish()
    }

    fn __autarkie_id_name() -> String {
        std::any::type_name::<Self>().to_string()
    }

    fn __autarkie_id_tuple() -> (Id, String) {
        (Self::__autarkie_id(), Self::__autarkie_id_name())
    }

    fn inner_id() -> Id {
        Self::__autarkie_id()
    }

    fn autarkie_deserialize(data: &mut &[u8]) -> Option<Self> {
        crate::maybe_deserialize(data)
    }

    fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {}

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {}

    fn __autarkie_node_ty(&self, visitor: &Visitor) -> NodeType {
        NodeType::NonRecursive
    }

    fn __autarkie_serialized(&self, visitor: &mut Visitor){}

    fn __autarkie_mutate(&mut self, ty: &mut MutationType, visitor: &mut Visitor, path: VecDeque<usize>) {
        debug_assert!(path.len() == 0);
        match ty {
            MutationType::Splice(other) => {
                *self = deserialize(other);
            }
            MutationType::GenerateReplace(ref mut bias) => {
                if let Some(generated) = Self::__autarkie_generate(visitor, bias, 0, None) {
                    *self = generated;
                    visitor.add_serialized(serialize(self), Self::__autarkie_id());
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

#[cfg(not(feature = "scale"))]
node!(serde::ser::Serialize + DeserializeOwned + 'static);

#[cfg(feature = "scale")]
node!(parity_scale_codec::Encode + parity_scale_codec::Decode + 'static);

impl<T: 'static> Node for PhantomData<T> {
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: usize,
        settings: Option<GenerateSettings>,
    ) -> Option<Self> {
        Some(Self)
    }
}

impl<T: 'static + Node + Clone> Node for Cow<'static, T> {
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: usize,
        settings: Option<GenerateSettings>,
    ) -> Option<Self> {
        Some(Cow::Owned(T::__autarkie_generate(
            visitor, depth, cur_depth, None,
        )?))
    }
    fn __autarkie_serialized(&self, visitor: &mut Visitor) {
        self.as_ref().__autarkie_serialized(visitor);
    }
    // TODO: fields / mutate
}

impl<T> Node for Cow<'static, [T]>
where
    T: Node + Clone,
{
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: usize,
        settings: Option<GenerateSettings>,
    ) -> Option<Self> {
        let element_count = if let Some(GenerateSettings::Length(len)) = settings {
            len
        } else if let Some(GenerateSettings::Range(range)) = settings {
            visitor.random_range(*range.start(), *range.end() + 1)
        } else {
            visitor.random_range(0, visitor.iterate_depth())
        };
        if element_count == 0 {
            return Some(vec![].into());
        }
        let mut vector = Vec::with_capacity(element_count);
        for i in 0..element_count {
            vector.push(T::__autarkie_generate(visitor, &mut 0, cur_depth, None)?);
        }
        Some(vector.into())
    }

    fn __autarkie_register(v: &mut Visitor, parent: Option<(Id, String)>, variant: usize) {
        if !v.is_recursive(T::__autarkie_id()) {
            T::__autarkie_register(v, parent, variant);
        } else {
            v.register_ty(parent, T::__autarkie_id_tuple(), variant);
            v.pop_ty();
        }
    }

    fn __autarkie_node_ty(&self, visitor: &Visitor) -> NodeType {
        NodeType::Iterable(false, self.len(), Self::inner_id())
    }

    fn inner_id() -> Id {
        T::__autarkie_id()
    }

    fn __autarkie_serialized(&self, visitor: &mut Visitor) {
        for item in self.as_ref() {
            visitor.add_serialized(serialize(&item), T::__autarkie_id());
            item.__autarkie_serialized(visitor);
        }
    }

    fn __autarkie_mutate(
        &mut self,
        ty: &mut MutationType,
        visitor: &mut Visitor,
        mut path: VecDeque<usize>,
    ) {
        if let Some(popped) = path.pop_front() {
            let mut cloned = self.as_ref().to_vec();
            cloned
                .get_mut(popped)
                .expect("UbEi1VMg____")
                .__autarkie_mutate(ty, visitor, path);
            *self = cloned.into();
        } else {
            match ty {
                MutationType::Splice(other) => {
                    *self = deserialize(other);
                }
                MutationType::GenerateReplace(ref mut bias) => {
                    if let Some(generated) = Self::__autarkie_generate(visitor, bias, 0, None) {
                        *self = generated;
                        self.__autarkie_serialized(visitor);
                    }
                }
                MutationType::SpliceAppend(other) => {
                    // TODO: make more performant
                    let mut cloned = self.as_ref().to_vec();
                    cloned.push(deserialize(other));
                    *self = cloned.into();
                }
                MutationType::GenerateAppend(bias) => {
                    if let Some(generated) = T::__autarkie_generate(visitor, bias, 0, None) {
                        // TODO: make more performant
                        let mut cloned = self.as_ref().to_vec();
                        cloned.push(generated);
                        *self = cloned.into();
                    }
                }
                MutationType::IterablePop(ref mut bias) => {
                    // TODO: make more performant
                    let mut cloned = self.as_ref().to_vec();
                    cloned.remove(*bias);
                    *self = cloned.into();
                }
                MutationType::RecursiveReplace => {
                    // TODO
                }
            }
        }
    }

    fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {
        for (index, child) in self.iter().enumerate() {
            visitor.register_field_stack((
                (index, child.__autarkie_node_ty(visitor)),
                T::__autarkie_id(),
            ));
            child.__autarkie_fields(visitor, 0);
            visitor.pop_field();
        }
    }

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        for (index, child) in self.iter().enumerate() {
            visitor.register_field_stack((
                ((index, child.__autarkie_node_ty(visitor))),
                T::__autarkie_id(),
            ));
            child.__autarkie_cmps(visitor, index, val);
            visitor.pop_field();
        }
    }
}

#[cfg(feature = "scale")]
impl<T, const N: usize> Node for [T; N]
where
    T: Node,
{
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: usize,
        settings: Option<crate::GenerateSettings>,
    ) -> Option<Self> {
        Some(
            (0..N)
                .map(|_| T::__autarkie_generate(visitor, depth, cur_depth, None))
                .filter_map(|i| i)
                .collect::<Vec<T>>()
                .try_into()
                .ok()?,
        )
    }

    fn __autarkie_serialized(&self, visitor: &mut Visitor) {
        for item in self {
            visitor.add_serialized(serialize(&item), T::__autarkie_id());
            item.__autarkie_serialized(visitor);
        }
    }

    fn __autarkie_node_ty(&self, visitor: &Visitor) -> crate::NodeType {
        crate::NodeType::Iterable(true, N, T::__autarkie_id())
    }

    fn __autarkie_register(v: &mut Visitor, parent: Option<(crate::Id, String)>, variant: usize) {
        if !v.is_recursive(T::__autarkie_id()) {
            T::__autarkie_register(v, parent, variant);
        } else {
            v.register_ty(parent, T::__autarkie_id_tuple(), variant);
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
                    if let Some(generated) = Self::__autarkie_generate(visitor, bias, 0, None) {
                        *self = generated;
                        self.__autarkie_serialized(visitor);
                    }
                }
                _ => unreachable!("tAL6LPUb____"),
            }
        }
    }
    fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {
        for (index, child) in self.iter().enumerate() {
            visitor.register_field_stack((
                ((index, child.__autarkie_node_ty(visitor))),
                T::__autarkie_id(),
            ));
            child.__autarkie_fields(visitor, 0);
            visitor.pop_field();
        }
    }

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        for (index, child) in self.iter().enumerate() {
            visitor.register_field_stack((
                ((index, child.__autarkie_node_ty(visitor))),
                T::__autarkie_id(),
            ));
            child.__autarkie_cmps(visitor, index, val);
            visitor.pop_field();
        }
    }
}

impl<T> Node for Vec<T>
where
    T: Node,
{
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: usize,
        settings: Option<GenerateSettings>,
    ) -> Option<Self> {
        let element_count = if let Some(GenerateSettings::Length(len)) = settings {
            len
        } else if let Some(GenerateSettings::Range(range)) = settings {
            visitor.random_range(*range.start(), *range.end() + 1)
        } else {
            visitor.random_range(0, visitor.iterate_depth())
        };
        if element_count == 0 {
            return Some(vec![]);
        }
        let mut vector = Vec::with_capacity(element_count);
        for i in 0..element_count {
            vector.push(T::__autarkie_generate(visitor, &mut 0, cur_depth, None)?);
        }
        Some(vector)
    }

    fn __autarkie_register(v: &mut Visitor, parent: Option<(Id, String)>, variant: usize) {
        if !v.is_recursive(T::__autarkie_id()) {
            T::__autarkie_register(v, parent, variant);
        } else {
            v.register_ty(parent, T::__autarkie_id_tuple(), variant);
            v.pop_ty();
        }
    }

    fn __autarkie_node_ty(&self, visitor: &Visitor) -> NodeType {
        NodeType::Iterable(false, self.len(), Self::inner_id())
    }

    fn inner_id() -> Id {
        T::__autarkie_id()
    }

    fn __autarkie_serialized(&self, visitor: &mut Visitor) {
        for item in self {
            visitor.add_serialized(serialize(&item), T::__autarkie_id());
            item.__autarkie_serialized(visitor);
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
                .expect("UbEi1VMg____")
                .__autarkie_mutate(ty, visitor, path);
        } else {
            match ty {
                MutationType::Splice(other) => {
                    *self = deserialize(other);
                }
                MutationType::GenerateReplace(ref mut bias) => {
                    if let Some(generated) = Self::__autarkie_generate(visitor, bias, 0, None) {
                        *self = generated;
                        self.__autarkie_serialized(visitor);
                    }
                }
                MutationType::SpliceAppend(other) => {
                    self.push(deserialize(other));
                }
                MutationType::GenerateAppend(bias) => {
                    if let Some(generated) = T::__autarkie_generate(visitor, bias, 0, None) {
                        self.push(generated)
                    }
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
            visitor.register_field_stack((
                (index, child.__autarkie_node_ty(visitor)),
                T::__autarkie_id(),
            ));
            child.__autarkie_fields(visitor, 0);
            visitor.pop_field();
        }
    }

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        for (index, child) in self.iter().enumerate() {
            visitor.register_field_stack((
                ((index, child.__autarkie_node_ty(visitor))),
                T::__autarkie_id(),
            ));
            child.__autarkie_cmps(visitor, index, val);
            visitor.pop_field();
        }
    }
}

impl Node for bool {
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: usize,
        settings: Option<GenerateSettings>,
    ) -> Option<Self> {
        Some(visitor.coinflip())
    }
}

impl<T> Node for Box<T>
where
    T: Node + Clone,
{
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: usize,
        settings: Option<GenerateSettings>,
    ) -> Option<Self> {
        Some(Box::new(T::__autarkie_generate(
            visitor, depth, cur_depth, None,
        )?))
    }

    fn inner_id() -> Id {
        T::__autarkie_id()
    }

    fn __autarkie_register(v: &mut Visitor, parent: Option<(Id, String)>, variant: usize) {
        if !v.is_recursive(T::__autarkie_id()) {
            T::__autarkie_register(v, parent, variant);
        } else {
            v.register_ty(parent, T::__autarkie_id_tuple(), variant);
            v.pop_ty();
        }
    }

    fn __autarkie_node_ty(&self, visitor: &Visitor) -> NodeType {
        self.as_ref().__autarkie_node_ty(visitor)
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

    fn __autarkie_serialized(&self, visitor: &mut Visitor) {
        self.as_ref().__autarkie_serialized(visitor)
    }
}

impl<T> Node for Option<T>
where
    T: Node,
{
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: usize,
        settings: Option<GenerateSettings>,
    ) -> Option<Self> {
        let choose_some = visitor.coinflip();
        if choose_some {
            Some(Some(T::__autarkie_generate(
                visitor, depth, cur_depth, None,
            )?))
        } else {
            Some(None)
        }
    }

    fn inner_id() -> Id {
        T::__autarkie_id()
    }
    // PhantomData<bool> is used as a dummy value for "None"
    fn __autarkie_register(v: &mut Visitor, parent: Option<(Id, String)>, variant: usize) {
        v.register_ty(parent, Self::__autarkie_id_tuple(), variant);
        if !v.is_recursive(T::__autarkie_id()) {
            T::__autarkie_register(v, Some(Self::__autarkie_id_tuple()), 0);
            v.register_ty(
                Some(Self::__autarkie_id_tuple()),
                PhantomData::<bool>::__autarkie_id_tuple(),
                1,
            );
            v.pop_ty();
        } else {
            v.register_ty(
                Some(Self::__autarkie_id_tuple()),
                T::__autarkie_id_tuple(),
                0,
            );
            v.pop_ty();
            v.register_ty(
                Some(Self::__autarkie_id_tuple()),
                PhantomData::<bool>::__autarkie_id_tuple(),
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
                    if let Some(generated) = Self::__autarkie_generate(visitor, bias, 0, None) {
                        *self = generated;
                        visitor.add_serialized(serialize(self), Self::__autarkie_id());
                        self.__autarkie_serialized(visitor);
                    }
                }
                _ => {
                    unreachable!()
                }
            }
        }
    }

    // TODO: for now we perform duplicate serialization cause the inner field is also serialized.
    // and our parent will serialize us
    fn __autarkie_serialized(&self, visitor: &mut Visitor) {
        let Some(inner) = self else {
            return;
        };
        visitor.add_serialized(serialize(&inner), T::__autarkie_id());
        inner.__autarkie_serialized(visitor);
    }

    fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {
        if let Some(inner) = self {
            visitor.register_field_stack((
                (index, inner.__autarkie_node_ty(visitor)),
                T::__autarkie_id(),
            ));
            inner.__autarkie_fields(visitor, 0);
            visitor.pop_field();
        }
    }

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        if let Some(inner) = self {
            visitor.register_field_stack((
                (index, inner.__autarkie_node_ty(visitor)),
                T::__autarkie_id(),
            ));
            inner.__autarkie_cmps(visitor, 0, val);
            visitor.pop_field();
        }
    }
}

// This is very similar to the derive implementation fr Enum,
// When things get fucked -> just look at this to save yourself from macro hell
impl<T, E> Node for Result<T, E>
where
    T: Node,
    E: Node,
{
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: usize,
        settings: Option<GenerateSettings>,
    ) -> Option<Self> {
        let choose_ok = visitor.coinflip();
        if choose_ok {
            Some(Ok(T::__autarkie_generate(visitor, depth, cur_depth, None)?))
        } else {
            Some(Err(E::__autarkie_generate(
                visitor, depth, cur_depth, None,
            )?))
        }
    }

    fn __autarkie_register(v: &mut Visitor, parent: Option<(Id, String)>, variant: usize) {
        v.register_ty(parent, Self::__autarkie_id_tuple(), variant);
        if !v.is_recursive(T::__autarkie_id()) {
            T::__autarkie_register(v, Some(Self::__autarkie_id_tuple()), 0);
        } else {
            v.register_ty(
                Some(Self::__autarkie_id_tuple()),
                T::__autarkie_id_tuple(),
                0,
            );
            v.pop_ty();
        }
        if !v.is_recursive(E::__autarkie_id()) {
            E::__autarkie_register(v, Some(Self::__autarkie_id_tuple()), 1);
        } else {
            v.register_ty(
                Some(Self::__autarkie_id_tuple()),
                E::__autarkie_id_tuple(),
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
                    if let Some(generated) = Self::__autarkie_generate(visitor, bias, 0, None) {
                        *self = generated;
                        visitor.add_serialized(serialize(self), Self::__autarkie_id());
                        self.__autarkie_serialized(visitor);
                    }
                }
                _ => {
                    unreachable!()
                }
            }
        }
    }

    fn __autarkie_serialized(&self, visitor: &mut Visitor) {
        if let Ok(inner) = self {
            visitor.add_serialized(serialize(&inner), T::__autarkie_id());
            inner.__autarkie_serialized(visitor);
        } else if let Err(inner) = self {
            visitor.add_serialized(serialize(&inner), E::__autarkie_id());
            inner.__autarkie_serialized(visitor);
        } else {
            unreachable!("zKJv3wsE____")
        }
    }

    fn __autarkie_fields(&self, visitor: &mut Visitor, index: usize) {
        visitor.register_field_stack((
            (index, self.__autarkie_node_ty(visitor)),
            Self::__autarkie_id(),
        ));
        if let Ok(inner) = self {
            inner.__autarkie_fields(visitor, 0);
        } else if let Err(inner) = self {
            inner.__autarkie_fields(visitor, 1);
        }
        visitor.pop_field();
    }

    fn __autarkie_cmps(&self, visitor: &mut Visitor, index: usize, val: (u64, u64)) {
        visitor.register_field_stack((
            (index, self.__autarkie_node_ty(visitor)),
            Self::__autarkie_id(),
        ));
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
        cur_depth: usize,
        settings: Option<GenerateSettings>,
    ) -> Option<Self> {
        Some(visitor.get_string())
    }
}

#[cfg(not(feature = "scale"))]
impl Node for char {
    fn __autarkie_generate(
        visitor: &mut Visitor,
        depth: &mut usize,
        cur_depth: usize,
        settings: Option<GenerateSettings>,
    ) -> Option<Self> {
        Some(
            char::from_u32(
                u32::__autarkie_generate(visitor, depth, cur_depth, None).expect("bHh7B75Y____"),
            )
            .unwrap_or_default(),
        )
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
        cur_depth: usize,
        settings: Option<GenerateSettings>,
    ) -> Option<Self> {
        Some(BTreeMap::new())
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

    fn __autarkie_node_ty(&self, visitor: &Visitor) -> NodeType {
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
                        let Some(key) = K::__autarkie_generate(visitor, bias, 0, None) else {
                            return;
                        };
                        let Some(val) = V::__autarkie_generate(visitor, bias, 0, None) else {
                            return;
                        };
                        self.insert(key, val);
                        self.__autarkie_serialized(visitor);
                        visitor.add_serialized(serialize(self), Self::__autarkie_id());
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
                    if let Some(generated) = Self::__autarkie_generate(visitor, bias, 0, None) {
                        *self = generated;
                        self.__autarkie_serialized(visitor);
                        visitor.add_serialized(serialize(self), Self::__autarkie_id());
                    }
                }
                MutationType::SpliceAppend(other) => {
                    let (k, v) = deserialize(other);
                    self.insert(k, v);
                }
                MutationType::GenerateAppend(bias) => {
                    if let Some(k) = K::__autarkie_generate(visitor, bias, 0, None) {
                        if let Some(v) = V::__autarkie_generate(visitor, bias, 0, None) {
                            self.insert(k, v);
                        }
                    }
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

    fn __autarkie_serialized(&self, visitor: &mut Visitor) {
        for (k, v) in self {
            visitor.add_serialized(serialize(&k), K::__autarkie_id());
            k.__autarkie_serialized(visitor);
            visitor.add_serialized(serialize(&v), V::__autarkie_id());
            v.__autarkie_serialized(visitor);
        }
    }
}

macro_rules! tuple_impls {
    ( $( ($T:ident , $id:tt)),+ ) => {
        impl<$($T: Node),+> Node for ($($T,)+)
        {
            fn __autarkie_generate(
                visitor: &mut Visitor,
                depth: &mut usize, cur_depth : usize,
                settings: Option<GenerateSettings>
            ) -> Option<Self> {
                Some(($($T::__autarkie_generate(visitor, depth, cur_depth, None)?,)+))
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
                            if let Some(generated) = Self::__autarkie_generate(visitor, bias, 0, None ) {
                            *self = generated;
                            self.__autarkie_serialized(visitor);
                            visitor.add_serialized(serialize(self), Self::__autarkie_id());
        }
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

            fn __autarkie_register(v: &mut Visitor, parent: Option<(Id, String)>, variant: usize) {
                v.register_ty(parent, Self::__autarkie_id_tuple(), variant);
                $({
                if !v.is_recursive($T::__autarkie_id()) {
                    $T::__autarkie_register(v, Some(Self::__autarkie_id_tuple()), 0);
                } else {
                    v.register_ty(Some(Self::__autarkie_id_tuple()), $T::__autarkie_id_tuple(), 0);
                    v.pop_ty();
                }
                })*
                v.pop_ty();
            }

            fn __autarkie_serialized(&self, visitor: &mut Visitor, ) {
                    $({
                        visitor.add_serialized(serialize(&self.$id), $T::__autarkie_id());
                        self.$id.__autarkie_serialized(visitor);
                    })*
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
                cur_depth: usize,
                settings: Option<GenerateSettings>,
            ) -> Option<Self> {
                let mut res = deserialize::<Self>(&mut v.generate_bytes($num_bytes).as_slice());
                if let Some(GenerateSettings::Range(range)) = settings {
                    res = res % (*range.end() as Self);
                    if res < *range.start() as Self {
                        res = (*range.start() as Self);
                    }
                };
                Some(res)
            }
            fn __autarkie_cmps(&self, v: &mut Visitor, index: usize, val: (u64, u64)) {
                if val.0 == *self as u64 {
                    v.register_cmp(serialize(&(val.1 as Self)));
                } else if val.1 == *self as u64 {
                    v.register_cmp(serialize(&(val.0 as Self)));
                }
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
#[cfg(not(feature = "scale"))]
impl_generate_simple!(isize, 8);
#[cfg(not(feature = "scale"))]
impl_generate_simple!(usize, 8);

#[cfg(not(feature = "scale"))]
pub fn serialize<T>(data: &T) -> Vec<u8>
where
    T: serde::Serialize,
{
    bincode::serialize(data).expect("invariant; we must always be able to serialize")
}

#[cfg(not(feature = "scale"))]
pub fn deserialize<T>(data: &mut &[u8]) -> T
where
    T: DeserializeOwned,
{
    crate::maybe_deserialize(data).expect("invariant; we must always be able to deserialize")
}

#[cfg(not(feature = "scale"))]
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
pub fn serialize_vec_len(len: usize) -> Vec<u8> {
    use parity_scale_codec::Encode;
    (parity_scale_codec::Compact(len as u32)).encode()
}

#[cfg(feature = "scale")]
pub fn maybe_deserialize<T>(data: &mut &[u8]) -> Option<T>
where
    T: parity_scale_codec::Decode,
{
    let Ok(res) = T::decode(data) else {
        return None;
    };

    Some(res)
}
#[cfg(feature = "scale")]
pub fn deserialize<T>(data: &mut &[u8]) -> T
where
    T: parity_scale_codec::Decode,
{
    crate::maybe_deserialize(data).expect("invariant; we must always be able to deserialize")
}

#[cfg(not(feature = "scale"))]
pub fn maybe_deserialize<T>(data: &mut &[u8]) -> Option<T>
where
    T: DeserializeOwned,
{
    let Ok(res) = bincode::deserialize(data) else {
        return None;
    };
    Some(res)
}
