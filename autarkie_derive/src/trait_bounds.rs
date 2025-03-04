// Copyright 2019 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::iter;

use proc_macro2::Ident;
use syn::{
    parse_quote,
    spanned::Spanned,
    visit::{self, Visit},
    Generics, Result, Type, TypePath,
};

use crate::utils::{self};

/// Visits the ast and checks if one of the given idents is found.
struct ContainIdents<'a> {
    result: bool,
    idents: &'a [Ident],
}

impl<'ast> Visit<'ast> for ContainIdents<'_> {
    fn visit_ident(&mut self, i: &'ast Ident) {
        if self.idents.iter().any(|id| id == i) {
            self.result = true;
        }
    }
}

/// Checks if the given type contains one of the given idents.
fn type_contain_idents(ty: &Type, idents: &[Ident]) -> bool {
    let mut visitor = ContainIdents {
        result: false,
        idents,
    };
    visitor.visit_type(ty);
    visitor.result
}

/// Visits the ast and checks if the a type path starts with the given ident.
struct TypePathStartsWithIdent<'a> {
    result: bool,
    ident: &'a Ident,
}

impl<'ast> Visit<'ast> for TypePathStartsWithIdent<'_> {
    fn visit_type_path(&mut self, i: &'ast TypePath) {
        if let Some(segment) = i.path.segments.first() {
            if &segment.ident == self.ident {
                self.result = true;
                return;
            }
        }

        visit::visit_type_path(self, i);
    }
}

/// Checks if the given type path or any containing type path starts with the given ident.
fn type_path_or_sub_starts_with_ident(ty: &TypePath, ident: &Ident) -> bool {
    let mut visitor = TypePathStartsWithIdent {
        result: false,
        ident,
    };
    visitor.visit_type_path(ty);
    visitor.result
}

/// Checks if the given type or any containing type path starts with the given ident.
fn type_or_sub_type_path_starts_with_ident(ty: &Type, ident: &Ident) -> bool {
    let mut visitor = TypePathStartsWithIdent {
        result: false,
        ident,
    };
    visitor.visit_type(ty);
    visitor.result
}

/// Visits the ast and collects all type paths that do not start or contain the given ident.
///
/// Returns `T`, `N`, `A` for `Vec<(Recursive<T, N>, A)>` with `Recursive` as ident.
struct FindTypePathsNotStartOrContainIdent<'a> {
    result: Vec<TypePath>,
    ident: &'a Ident,
}

impl<'ast> Visit<'ast> for FindTypePathsNotStartOrContainIdent<'_> {
    fn visit_type_path(&mut self, i: &'ast TypePath) {
        if type_path_or_sub_starts_with_ident(i, self.ident) {
            visit::visit_type_path(self, i);
        } else {
            self.result.push(i.clone());
        }
    }
}

/// Collects all type paths that do not start or contain the given ident in the given type.
///
/// Returns `T`, `N`, `A` for `Vec<(Recursive<T, N>, A)>` with `Recursive` as ident.
fn find_type_paths_not_start_or_contain_ident(ty: &Type, ident: &Ident) -> Vec<TypePath> {
    let mut visitor = FindTypePathsNotStartOrContainIdent {
        result: Vec::new(),
        ident,
    };
    visitor.visit_type(ty);
    visitor.result
}

pub fn add(input_ident: &Ident, generics: &mut Generics, data: &syn::Data) {
    let ty_params = generics
        .type_params()
        .map(|tp| tp.ident.clone())
        .collect::<Vec<_>>();
    let types_with_bounds =
        get_types_to_add_trait_bound(input_ident, data, &ty_params, false).unwrap();
    let where_clause = generics.make_where_clause();
    types_with_bounds.into_iter().for_each(|ty| {
        where_clause
            .predicates
            .push(parse_quote!(#ty : ::autarkie::Node))
    });
}
/// Returns all types that must be added to the where clause with the respective trait bound.
pub fn get_types_to_add_trait_bound(
    input_ident: &Ident,
    data: &syn::Data,
    ty_params: &[Ident],
    dumb_trait_bound: bool,
) -> Result<Vec<Type>> {
    if dumb_trait_bound {
        Ok(ty_params.iter().map(|t| parse_quote!( #t )).collect())
    } else {
        let needs_codec_bound = |f: &syn::Field| {
            !utils::is_compact(f)
                && utils::get_encoded_as_type(f).is_none()
                && !utils::should_skip(&f.attrs)
        };
        let res = collect_types(data, needs_codec_bound)?
            .into_iter()
            // Only add a bound if the type uses a generic
            .filter(|ty| type_contain_idents(ty, ty_params))
            // If a struct contains itself as field type, we can not add this type into the where
            // clause. This is required to work a round the following compiler bug: https://github.com/rust-lang/rust/issues/47032
            .flat_map(|ty| {
                find_type_paths_not_start_or_contain_ident(&ty, input_ident)
                    .into_iter()
                    .map(Type::Path)
                    // Remove again types that do not contain any of our generic parameters
                    .filter(|ty| type_contain_idents(ty, ty_params))
                    // Add back the original type, as we don't want to loose it.
                    .chain(iter::once(ty))
            })
            // Remove all remaining types that start/contain the input ident to not have them in the
            // where clause.
            .filter(|ty| !type_or_sub_type_path_starts_with_ident(ty, input_ident))
            .collect();

        Ok(res)
    }
}

fn collect_types(data: &syn::Data, type_filter: fn(&syn::Field) -> bool) -> Result<Vec<syn::Type>> {
    use syn::*;

    let types = match *data {
        Data::Struct(ref data) => match &data.fields {
            Fields::Named(FieldsNamed { named: fields, .. })
            | Fields::Unnamed(FieldsUnnamed {
                unnamed: fields, ..
            }) => fields
                .iter()
                .filter(|f| type_filter(f))
                .map(|f| f.ty.clone())
                .collect(),

            Fields::Unit => Vec::new(),
        },

        Data::Enum(ref data) => data
            .variants
            .iter()
            .filter(|variant| !utils::should_skip(&variant.attrs))
            .flat_map(|variant| match &variant.fields {
                Fields::Named(FieldsNamed { named: fields, .. })
                | Fields::Unnamed(FieldsUnnamed {
                    unnamed: fields, ..
                }) => fields
                    .iter()
                    .filter(|f| type_filter(f))
                    .map(|f| f.ty.clone())
                    .collect(),

                Fields::Unit => Vec::new(),
            })
            .collect(),

        Data::Union(ref data) => {
            return Err(Error::new(
                data.union_token.span(),
                "Union types are not supported.",
            ))
        }
    };

    Ok(types)
}
