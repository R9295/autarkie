// Copyright 2018-2020 Parity Technologies
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

//! Various internal utils.
//!
//! NOTE: attributes finder must be checked using check_attribute first,
//! otherwise the macro can panic.

use std::str::FromStr;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::Parse, parse_quote, punctuated::Punctuated, spanned::Spanned, token, Attribute, Expr,
    ExprLit, Field, Lit, Meta, Path, Token, Variant,
};

fn find_meta_item<'a, F, R, I, M>(mut itr: I, mut pred: F) -> Option<R>
where
    F: FnMut(M) -> Option<R> + Clone,
    I: Iterator<Item = &'a Attribute>,
    M: Parse,
{
    itr.find_map(|attr| {
        attr.path()
            .is_ident("codec")
            .then(|| pred(attr.parse_args().ok()?))
            .flatten()
    })
}

/// Look for a `#[scale(index = $int)]` attribute on a variant. If no attribute
/// is found, fall back to the discriminant or just the variant index.
pub fn variant_index(v: &Variant, i: usize) -> TokenStream {
    // first look for an attribute
    let index = find_meta_item(v.attrs.iter(), |meta| {
        if let Meta::NameValue(ref nv) = meta {
            if nv.path.is_ident("index") {
                if let Expr::Lit(ExprLit {
                    lit: Lit::Int(ref v),
                    ..
                }) = nv.value
                {
                    let byte = v
                        .base10_parse::<u8>()
                        .expect("Internal error, index attribute must have been checked");
                    return Some(byte);
                }
            }
        }

        None
    });

    // then fallback to discriminant or just index
    index.map(|i| quote! { #i }).unwrap_or_else(|| {
        v.discriminant
            .as_ref()
            .map(|(_, expr)| quote! { #expr })
            .unwrap_or_else(|| quote! { #i })
    })
}

/// Look for a `#[codec(encoded_as = "SomeType")]` outer attribute on the given
/// `Field`.
pub fn get_encoded_as_type(field: &Field) -> Option<TokenStream> {
    find_meta_item(field.attrs.iter(), |meta| {
        if let Meta::NameValue(ref nv) = meta {
            if nv.path.is_ident("encoded_as") {
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(ref s),
                    ..
                }) = nv.value
                {
                    return Some(
                        TokenStream::from_str(&s.value())
                            .expect("Internal error, encoded_as attribute must have been checked"),
                    );
                }
            }
        }

        None
    })
}

/// Look for a `#[codec(compact)]` outer attribute on the given `Field`. If the attribute is found,
/// return the compact type associated with the field type.
pub fn get_compact_type(field: &Field, crate_path: &syn::Path) -> Option<TokenStream> {
    find_meta_item(field.attrs.iter(), |meta| {
        if let Meta::Path(ref path) = meta {
            if path.is_ident("compact") {
                let field_type = &field.ty;
                return Some(quote! {<#field_type as #crate_path::HasCompact>::Type});
            }
        }

        None
    })
}

/// Look for a `#[codec(compact)]` outer attribute on the given `Field`.
pub fn is_compact(field: &Field) -> bool {
    get_compact_type(field, &parse_quote!(::crate)).is_some()
}

/// Look for a `#[codec(skip)]` in the given attributes.
pub fn should_skip(attrs: &[Attribute]) -> bool {
    find_meta_item(attrs.iter(), |meta| {
        if let Meta::Path(ref path) = meta {
            if path.is_ident("skip") {
                return Some(path.span());
            }
        }

        None
    })
    .is_some()
}

fn crate_access() -> syn::Result<proc_macro2::Ident> {
    use proc_macro2::{Ident, Span};
    use proc_macro_crate::{crate_name, FoundCrate};
    const DEF_CRATE: &str = "thesis";
    match crate_name(DEF_CRATE) {
        Ok(FoundCrate::Itself) => {
            let name = DEF_CRATE.to_string().replace('-', "_");
            Ok(syn::Ident::new(&name, Span::call_site()))
        }
        Ok(FoundCrate::Name(name)) => Ok(Ident::new(&name, Span::call_site())),
        Err(e) => Err(syn::Error::new(Span::call_site(), e)),
    }
}
/// This struct matches `crate = ...` where the ellipsis is a `Path`.
struct CratePath {
    _crate_token: Token![crate],
    _eq_token: Token![=],
    path: Path,
}

impl Parse for CratePath {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(CratePath {
            _crate_token: input.parse()?,
            _eq_token: input.parse()?,
            path: input.parse()?,
        })
    }
}

impl From<CratePath> for Path {
    fn from(CratePath { path, .. }: CratePath) -> Self {
        path
    }
}

/// Match `#[codec(crate = ...)]` and return the `...` if it is a `Path`.
fn codec_crate_path_inner(attr: &Attribute) -> Option<Path> {
    // match `#[codec ...]`
    attr.path()
        .is_ident("codec")
        .then(|| {
            // match `#[codec(crate = ...)]` and return the `...`
            attr.parse_args::<CratePath>().map(Into::into).ok()
        })
        .flatten()
}

/// Match `#[codec(crate = ...)]` and return the ellipsis as a `Path`.
///
/// If not found, returns the default crate access pattern.
///
/// If multiple items match the pattern, all but the first are ignored.
pub fn codec_crate_path(attrs: &[Attribute]) -> syn::Result<Path> {
    match attrs.iter().find_map(codec_crate_path_inner) {
        Some(path) => Ok(path),
        None => crate_access().map(|ident| parse_quote!(::#ident)),
    }
}

/// Parse `name(T: Bound, N: Bound)` or `name(skip_type_params(T, N))` as a custom trait bound.
pub enum CustomTraitBound<N> {
    SpecifiedBounds {
        _name: N,
        _paren_token: token::Paren,
        bounds: Punctuated<syn::WherePredicate, Token![,]>,
    },
    SkipTypeParams {
        _name: N,
        _paren_token_1: token::Paren,
        _skip_type_params: skip_type_params,
        _paren_token_2: token::Paren,
        type_names: Punctuated<syn::Ident, Token![,]>,
    },
}

impl<N: Parse> Parse for CustomTraitBound<N> {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut content;
        let _name: N = input.parse()?;
        let _paren_token = syn::parenthesized!(content in input);
        if content.peek(skip_type_params) {
            Ok(Self::SkipTypeParams {
                _name,
                _paren_token_1: _paren_token,
                _skip_type_params: content.parse::<skip_type_params>()?,
                _paren_token_2: syn::parenthesized!(content in content),
                type_names: content.parse_terminated(syn::Ident::parse, Token![,])?,
            })
        } else {
            Ok(Self::SpecifiedBounds {
                _name,
                _paren_token,
                bounds: content.parse_terminated(syn::WherePredicate::parse, Token![,])?,
            })
        }
    }
}

syn::custom_keyword!(encode_bound);
syn::custom_keyword!(decode_bound);
syn::custom_keyword!(decode_with_mem_tracking_bound);
syn::custom_keyword!(mel_bound);
syn::custom_keyword!(skip_type_params);
