use std::iter;

use proc_macro2::Ident;
use syn::{
    parse_quote,
    spanned::Spanned,
    visit::{self, Visit},
    Generics, Result, Type, TypePath,
};

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

fn type_contain_idents(ty: &Type, idents: &[Ident]) -> bool {
    let mut visitor = ContainIdents {
        result: false,
        idents,
    };
    visitor.visit_type(ty);
    visitor.result
}

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

fn type_path_or_sub_starts_with_ident(ty: &TypePath, ident: &Ident) -> bool {
    let mut visitor = TypePathStartsWithIdent {
        result: false,
        ident,
    };
    visitor.visit_type_path(ty);
    visitor.result
}

fn type_or_sub_type_path_starts_with_ident(ty: &Type, ident: &Ident) -> bool {
    let mut visitor = TypePathStartsWithIdent {
        result: false,
        ident,
    };
    visitor.visit_type(ty);
    visitor.result
}

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
    let types_with_bounds = get_types_to_add_trait_bound(input_ident, data, &ty_params).unwrap();
    let where_clause = generics.make_where_clause();
    types_with_bounds.into_iter().for_each(|ty| {
        where_clause
            .predicates
            .push(parse_quote!(#ty : ::autarkie::Node))
    });
}

fn get_types_to_add_trait_bound(
    input_ident: &Ident,
    data: &syn::Data,
    ty_params: &[Ident],
) -> Result<Vec<Type>> {
    let res = collect_types(data)?
        .into_iter()
        .filter(|ty| type_contain_idents(ty, ty_params))
        // Workaround for https://github.com/rust-lang/rust/issues/47032
        .flat_map(|ty| {
            find_type_paths_not_start_or_contain_ident(&ty, input_ident)
                .into_iter()
                .map(Type::Path)
                .filter(|ty| type_contain_idents(ty, ty_params))
                .chain(iter::once(ty))
        })
        .filter(|ty| !type_or_sub_type_path_starts_with_ident(ty, input_ident))
        .collect();

    Ok(res)
}

fn collect_types(data: &syn::Data) -> Result<Vec<syn::Type>> {
    use syn::*;

    let types = match *data {
        Data::Struct(ref data) => match &data.fields {
            Fields::Named(FieldsNamed { named: fields, .. })
            | Fields::Unnamed(FieldsUnnamed {
                unnamed: fields, ..
            }) => fields.iter().map(|f| f.ty.clone()).collect(),

            Fields::Unit => Vec::new(),
        },

        Data::Enum(ref data) => data
            .variants
            .iter()
            .flat_map(|variant| match &variant.fields {
                Fields::Named(FieldsNamed { named: fields, .. })
                | Fields::Unnamed(FieldsUnnamed {
                    unnamed: fields, ..
                }) => fields.iter().map(|f| f.ty.clone()).collect(),

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
