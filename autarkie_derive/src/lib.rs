extern crate proc_macro2;

use proc_macro::TokenStream;
use quote::quote;
mod trait_bounds;
use syn::{spanned::Spanned, token::Comma, *};

#[proc_macro_derive(
    Grammar,
    attributes(autarkie_literal, autarkie_length, autarkie_range, autarkie_no_mutate)
)]
pub fn derive_node(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut base_parsed = syn::parse_macro_input!(input as syn::DeriveInput);
    let root_name = &base_parsed.ident;
    let expanded = match base_parsed.data {
        Data::Struct(ref data) => {
            let fields = get_fields(&data.fields);
            let is_named = matches!(data.fields, syn::Fields::Named(_));
            let parsed = parse_fields(fields);
            let generate = construct_generate_function_struct(&parsed, is_named);

            let serialized_inner = parsed.iter().map(|field| {
                let name = field.get_name(is_named);
                let access = quote! { self.#name };
                construct_serialized_field(&field.ty, &access)
            });

            let serialized = quote! {
                autarkie_visitor.add_serialized(::autarkie::serialize(&self), Self::__autarkie_id());
                #(#serialized_inner)*
            };

            let register_field = parsed.iter().filter(|f| !f.no_mutate).map(|field| {
                let name = field.get_name(is_named);
                let access = quote! { self.#name };
                construct_field_visit_stmt(
                    field.id,
                    &field.ty,
                    &access,
                    &quote! { 0 },
                    FieldVisitKind::Fields,
                )
            });
            let register_cmps = parsed.iter().filter(|f| !f.no_mutate).map(|field| {
                let name = field.get_name(is_named);
                let access = quote! { self.#name };
                construct_field_visit_stmt(
                    field.id,
                    &field.ty,
                    &access,
                    &quote! { 0 },
                    FieldVisitKind::Cmps,
                )
            });

            let register_ty = parsed
                .iter()
                .map(|field| construct_register_ty_field(&field.ty, &quote! { 0 }));

            let inner_mutate = parsed.iter().map(|field| {
                let name = field.get_name(is_named);
                let access = quote! { self.#name };
                construct_mutate_field_arm(field.id, &access)
            });
            trait_bounds::add(root_name, &mut base_parsed.generics, &base_parsed.data);
            let (impl_generics, ty_generics, where_clause) = base_parsed.generics.split_for_impl();
            // Generate the Node trait implementation for the Struct
            let node_impl = quote! {
                impl #impl_generics ::autarkie::Node for #root_name #ty_generics #where_clause {
                    fn __autarkie_generate(v: &mut autarkie::Visitor, depth: &mut usize, cur_depth : usize, settings: Option<autarkie::GenerateSettings>) -> Option<Self> {
                        let (_, is_recursive) = v.generate(&Self::__autarkie_id(), cur_depth)?;
                        #generate
                    }

                    fn __autarkie_register(v: &mut ::autarkie::Visitor, parent: Option<(::autarkie::tree::Id, String)>, variant: usize) {
                        v.register_ty(parent, Self::__autarkie_id_tuple(), variant);
                        #(#register_ty)*;
                        v.pop_ty();
                    }

                    fn __autarkie_fields(&self, v: &mut ::autarkie::Visitor, __autarkie_index: usize) {
                        #(#register_field)*;
                    }


                    fn __autarkie_cmps(&self, v: &mut ::autarkie::Visitor, __autarkie_index: usize, __autarkie_val: (u64, u64)) {
                        #(#register_cmps)*
                    }

                    fn __autarkie_serialized(&self, autarkie_visitor: &mut ::autarkie::Visitor) {
                        #serialized
                    }

                    fn __autarkie_mutate(&mut self,
                        autarkie_ty: &mut autarkie::MutationType,
                        autarkie_visitor: &mut autarkie::Visitor,
                        mut autarkie_path: std::collections::VecDeque<usize>
                    ) {
                        if let Some(popped) = autarkie_path.pop_front() {
                            match popped {
                                #(#inner_mutate)*
                                _ => {
                                    unreachable!("____VzKs1CWu0S")
                                }
                            }
                        } else {
                            match autarkie_ty {
                                autarkie::MutationType::Splice(other) => {
                                    *self = autarkie::deserialize(other);
                                }
                                autarkie::MutationType::GenerateReplace(ref mut bias) => {
                                    if let Some(generated) = Self::__autarkie_generate(autarkie_visitor, bias, 0, None) {
                                        *self = generated;
                                        autarkie_visitor.add_serialized(autarkie::serialize(&self), Self::__autarkie_id());
                                        self.__autarkie_serialized(autarkie_visitor);
                                    }
                                }
                                _  => {
                                    unreachable!()
                                }
                            }
                        }
                    }
                };
            };

            quote! {
                #node_impl
            }
        }
        Data::Enum(ref data) => {
            let mut generate = vec![];
            let mut fn_fields = vec![];
            let mut inner_mutate = vec![];
            let mut fn_cmps = vec![];
            let mut are_we_recursive = vec![];
            let mut register_ty = vec![];
            let mut serialized_inner = vec![];

            for (i, variant) in data.variants.iter().enumerate() {
                let variant_name = &variant.ident;
                let fields = parse_fields(get_fields(&variant.fields));
                let is_named = matches!(variant.fields, syn::Fields::Named(_));
                are_we_recursive.push(if !fields.is_empty() {
                    if is_named {
                        quote! {#root_name::#variant_name{..} => {
                            if autarkie_visitor.is_recursive_variant(Self::__autarkie_id(), #i) {
                             autarkie::visitor::NodeType::Recursive
                            } else {
                             autarkie::visitor::NodeType::NonRecursive
                            }
                        }}
                    } else {
                        quote! {#root_name::#variant_name(..) => {
                            if autarkie_visitor.is_recursive_variant(Self::__autarkie_id(), #i) {
                             autarkie::visitor::NodeType::Recursive
                            } else {
                             autarkie::visitor::NodeType::NonRecursive
                            }
                        }}
                    }
                } else {
                    quote! {
                        #root_name::#variant_name {} => autarkie::visitor::NodeType::NonRecursive
                    }
                });

                let constructor =
                    construct_generate_function_enum(&fields, is_named, root_name, variant_name);
                generate.push(quote! {
                    #i => {
                        #constructor
                    }
                });

                fn_fields.push(Some(construct_enum_fields_like_arm(
                    root_name,
                    variant_name,
                    i,
                    &fields,
                    is_named,
                    FieldVisitKind::Fields,
                )));
                if fields.is_empty() {
                    register_ty.push(quote! {
                    // use something besides bool; bool is just a place holder.
                    v.register_ty(Some(Self::__autarkie_id_tuple()), <std::marker::PhantomData<bool>>::__autarkie_id_tuple(), #i);
                    v.pop_ty();
                });
                } else {
                    let register_fields = fields
                        .iter()
                        .map(|field| construct_register_ty_field(&field.ty, &quote! { #i }));
                    register_ty.push(quote! {#(#register_fields)*});
                }

                fn_cmps.push(Some(construct_enum_fields_like_arm(
                    root_name,
                    variant_name,
                    i,
                    &fields,
                    is_named,
                    FieldVisitKind::Cmps,
                )));
                inner_mutate.push(Some(construct_enum_mutate_arm(
                    root_name,
                    variant_name,
                    i,
                    &fields,
                    is_named,
                )));
                if !fields.is_empty() {
                    serialized_inner.push(construct_enum_serialized_arm(
                        root_name,
                        variant_name,
                        &fields,
                        is_named,
                    ));
                }
            }

            let generate_func = if data.variants.is_empty() {
                quote! {
                    Self {}
                }
            } else {
                let variant_id_calculation = {
                    quote! {
                        let (variant_id, is_recursive) = v.generate(&Self::__autarkie_id(), cur_depth)?;
                    }
                };
                quote! {
                        #variant_id_calculation
                        match variant_id {
                             #(#generate,)*
                            _ => unreachable!()
                        }
                }
            };
            trait_bounds::add(root_name, &mut base_parsed.generics, &base_parsed.data);
            let (impl_generics, ty_generics, where_clause) = base_parsed.generics.split_for_impl();
            // Generate the Node trait implementation for the Enum
            // TODO: can optimize this if the enum has only two variants like (Result)
            let node_impl = quote! {
                impl #impl_generics ::autarkie::Node for #root_name #ty_generics #where_clause {
                    fn __autarkie_generate(v: &mut ::autarkie::Visitor, depth: &mut usize, cur_depth : usize, settings: Option<autarkie::GenerateSettings>) -> Option<Self> {
                        #generate_func
                    }

                    fn __autarkie_fields(&self, v: &mut ::autarkie::Visitor, __autarkie_index: usize) {
                        #(#fn_fields)*;
                    }

                    fn __autarkie_register(v: &mut ::autarkie::Visitor, parent: Option<(::autarkie::tree::Id, String)>, variant: usize) {
                        v.register_ty(parent, Self::__autarkie_id_tuple(), variant);
                        #(#register_ty)*;
                        v.pop_ty();
                    }

                    fn __autarkie_cmps(&self, v: &mut ::autarkie::Visitor, __autarkie_index: usize, __autarkie_val: (u64, u64)) {
                        #(#fn_cmps)*;
                    }

                    fn __autarkie_serialized(&self, autarkie_visitor: &mut ::autarkie::Visitor) {
                        #(#serialized_inner)*
                    }

                    fn __autarkie_node_ty(&self, autarkie_visitor: &autarkie::Visitor) -> autarkie::visitor::NodeType {
                        match self {
                            #(#are_we_recursive,)*
                        }
                    }

                    fn __autarkie_mutate(&mut self, autarkie_ty: &mut autarkie::MutationType, autarkie_visitor: &mut autarkie::Visitor, mut autarkie_path: std::collections::VecDeque<usize>) {
                        if let Some(popped) = autarkie_path.pop_front() {
                            match popped {
                                #(#inner_mutate)*
                                _ => unreachable!("____VpyAL0wN7m")
                            }
                        }
                        else {
                            match autarkie_ty {
                                autarkie::MutationType::Splice(other) => {
                                    *self = autarkie::deserialize(other);
                                }
                                autarkie::MutationType::GenerateReplace(ref mut bias) => {
                                    if let Some(generated) = Self::__autarkie_generate(autarkie_visitor, bias, 0, None) {
                                        *self = generated;
                                        autarkie_visitor.add_serialized(autarkie::serialize(&self), Self::__autarkie_id());
                                        self.__autarkie_serialized(autarkie_visitor);
                                    }
                                }
                                autarkie::MutationType::RecursiveReplace => {
                                    if self.__autarkie_node_ty(autarkie_visitor).is_recursive() {
                                        // 0 depth == always non-recursive
                                    if let Some(generated) = Self::__autarkie_generate(autarkie_visitor, &mut 0, 0, None) {
                                        *self = generated;
                                        autarkie_visitor.add_serialized(autarkie::serialize(&self), Self::__autarkie_id());
                                        self.__autarkie_serialized(autarkie_visitor);
                                    }
                                    }
                                }
                                _  => {
                                    unreachable!()
                                }
                            }
                        }
                    }
                }
            };
            quote! {
                #node_impl
            }
        }
        Data::Union(ref data) => {
            return syn::Error::new_spanned(
                &data.union_token,
                "Grammar derive does not support unions",
            )
            .to_compile_error()
            .into();
        }
    };
    TokenStream::from(expanded)
}

#[derive(Clone, Copy)]
enum FieldVisitKind {
    Fields,
    Cmps,
}

fn variant_pattern(
    root_name: &Ident,
    variant_name: &Ident,
    fields: &[GrammarField],
    is_named: bool,
) -> proc_macro2::TokenStream {
    let field_names = fields.iter().map(|field| {
        let name = &field.name;
        quote! { #name }
    });
    if is_named {
        quote! { #root_name::#variant_name { #(#field_names),* } }
    } else {
        quote! { #root_name::#variant_name(#(#field_names),*) }
    }
}

fn construct_field_visit_stmt(
    id: usize,
    ty: &Type,
    field_access: &proc_macro2::TokenStream,
    idx: &proc_macro2::TokenStream,
    kind: FieldVisitKind,
) -> proc_macro2::TokenStream {
    let call = match kind {
        FieldVisitKind::Fields => quote! {
            #field_access.__autarkie_fields(v, #idx);
        },
        FieldVisitKind::Cmps => quote! {
            #field_access.__autarkie_cmps(v, #idx, __autarkie_val);
        },
    };
    quote! {
        v.register_field(((#id, #field_access.__autarkie_node_ty(v)), <#ty>::__autarkie_id()));
        #call
        v.pop_field();
    }
}

fn construct_enum_fields_like_arm(
    root_name: &Ident,
    variant_name: &Ident,
    variant_idx: usize,
    fields: &[GrammarField],
    is_named: bool,
    kind: FieldVisitKind,
) -> proc_macro2::TokenStream {
    if fields.is_empty() {
        return quote! {
            if let #root_name::#variant_name{} = self {}
        };
    }

    let pattern = variant_pattern(root_name, variant_name, fields, is_named);
    let field_ops = fields.iter().filter(|f| !f.no_mutate).map(|field| {
        let access = {
            let name = &field.name;
            quote! { #name }
        };
        let idx = field.id;
        construct_field_visit_stmt(field.id, &field.ty, &access, &quote! { #idx }, kind)
    });

    quote! {
        if let #pattern = self {
            v.register_field_stack(((#variant_idx, self.__autarkie_node_ty(v)), Self::__autarkie_id()));
            #(#field_ops)*
            v.pop_field();
        }
    }
}

fn construct_serialized_field(
    ty: &Type,
    field_access: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        // todo: check fixed size
        if !matches!(#field_access.__autarkie_node_ty(autarkie_visitor), autarkie::visitor::NodeType::Iterable(..)) {
            autarkie_visitor.add_serialized(::autarkie::serialize(&#field_access), <#ty>::__autarkie_id());
        }
        #field_access.__autarkie_serialized(autarkie_visitor);
    }
}

fn construct_enum_serialized_arm(
    root_name: &Ident,
    variant_name: &Ident,
    fields: &[GrammarField],
    is_named: bool,
) -> proc_macro2::TokenStream {
    let pattern = variant_pattern(root_name, variant_name, fields, is_named);
    let serialized_fields = fields.iter().map(|field| {
        let access = {
            let name = &field.name;
            quote! { #name }
        };
        construct_serialized_field(&field.ty, &access)
    });
    quote! {
        if let #pattern = self {
            #(#serialized_fields)*
        }
    }
}

fn construct_mutate_field_arm(
    id: usize,
    field_access: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        #id => {
            #field_access.__autarkie_mutate(autarkie_ty, autarkie_visitor, autarkie_path);
        },
    }
}

fn construct_enum_mutate_arm(
    root_name: &Ident,
    variant_name: &Ident,
    variant_idx: usize,
    fields: &[GrammarField],
    is_named: bool,
) -> proc_macro2::TokenStream {
    if fields.is_empty() {
        return quote! {
            #variant_idx => unreachable!("____aNHh8Ap8"),
        };
    }

    let pattern = variant_pattern(root_name, variant_name, fields, is_named);
    let field_mutations = fields.iter().map(|field| {
        let access = {
            let name = &field.name;
            quote! { #name }
        };
        construct_mutate_field_arm(field.id, &access)
    });

    quote! {
        #variant_idx => {
            if let #pattern = self {
                if let Some(popped) = autarkie_path.pop_front() {
                    match popped {
                        #(#field_mutations)*
                        _ => {
                            unreachable!("____FU1zlV0c")
                        }
                    }
                } else {
                    unreachable!("____kTHVIHpB");
                }
            } else {unreachable!("iOoUo7jL____")}
        },
    }
}

fn construct_register_ty_field(
    ty: &Type,
    variant_idx: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        if !v.is_recursive(<#ty>::__autarkie_id()) {
            <#ty>::__autarkie_register(v, Some(Self::__autarkie_id_tuple()), #variant_idx);
        } else {
            v.register_ty(Some(Self::__autarkie_id_tuple()), <#ty>::__autarkie_id_tuple(), #variant_idx);
            v.pop_ty();
        }
    }
}

fn parse_fields(
    fields: Option<&syn::punctuated::Punctuated<syn::Field, Comma>>,
) -> Vec<GrammarField> {
    if fields.is_none() {
        return vec![];
    }
    let fields = fields.unwrap();
    let fields = fields
        .iter()
        .enumerate()
        .map(|(id, field)| {
            let ty = &field.ty;
            let name = match field.ident.clone() {
                Some(ident) => ident,
                None => Ident::new(&format!("_{id}"), field.span()),
            };
            let no_mutate = field
                .attrs
                .iter()
                .any(|a| a.path().is_ident("autarkie_no_mutate"));
            GrammarField {
                name,
                ty: ty.clone(),
                id,
                attrs: field.attrs.clone(),
                no_mutate,
            }
        })
        .collect::<Vec<_>>();
    fields
}

fn get_field_defs(fields: &[GrammarField]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|field| {
            let name = &field.name;
            let ty = &field.ty;
            let mut generator = None;
            // The generator is a closure that is run immediately.
            // This allows us to sepcify literals for a field.
            // TODO: maybe do some sanitization of literals
            for attr in &field.attrs {
                if attr.path().is_ident("autarkie_literal") {
                    let literals = attr
                        .parse_args_with(
                            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
                        )
                        .unwrap()
                        .into_iter()
                        .map(|expr| match expr {
                            syn::Expr::Lit(_) => expr,
                            syn::Expr::Path(ref path) if path.path.get_ident().is_some() => expr,
                            _ => {
                                panic!("autarkie_literal(..) needs literal or ident values");
                            }
                        })
                        .collect::<Vec<_>>();
                    if literals.is_empty() {
                        panic!("autarkie_literal(..) needs literal or ident values");
                    }
                    let literals_len = literals.len() - 1;
                    if literals_len == 0 {
                        let item = literals.first().unwrap();
                        generator = Some(quote! {
                            let #name = #item as #ty;
                        });
                    } else {
                        generator = Some(quote! {
                            let #name = || -> #ty {
                                let item = v.random_range(0, #literals_len);
                                let literals = [#(#literals),*];
                                literals[item] as #ty
                            }();
                        });
                    }
                } else if attr.path().is_ident("autarkie_length") {
                    let values = attr
                        .parse_args_with(
                            syn::punctuated::Punctuated::<syn::LitInt, syn::Token![,]>::parse_terminated,
                        )
                        .unwrap();
                    if values.len() != 1 {
                        panic!("autarkie_min_size(..) needs an unsigned integer literal value!");
                    }
                    let item = values.first().unwrap();
                    generator = Some(quote! {
                        let #name = <#ty>::__autarkie_generate(v, depth, if is_recursive {cur_depth + 1} else {cur_depth},
                        Some(autarkie::GenerateSettings::Length(#item))
                    )?;
                    });
                } else if attr.path().is_ident("autarkie_range") {
                    let range: syn::ExprRange = attr.parse_args().unwrap();
                    generator = Some(quote! {
                        let #name = <#ty>::__autarkie_generate(v, depth, if is_recursive {cur_depth + 1} else {cur_depth},
                        Some(autarkie::GenerateSettings::Range(#range))
                    )?;
                    });
                }
            }
            // If we do not have a literal attribute, we use the inner generate function of the type.
            if generator.is_none() {
                generator = Some(quote! {
                    let #name = <#ty>::__autarkie_generate(v, depth, if is_recursive {cur_depth + 1} else {cur_depth}, None)?;
                });
            }
            // this should never happen, cause we either have a literal attribute or not.
            generator
                .unwrap_or_else(|| panic!("invariant; field {name:?} did not have a generator"))
        })
        .collect::<Vec<_>>()
}

fn construct_generate_function_struct(
    fields: &[GrammarField],
    is_named: bool,
) -> proc_macro2::TokenStream {
    let field_defs = get_field_defs(fields);
    let names = fields.iter().map(|field| &field.name);
    // if the struct is
    // non named -> Struct(x, y, z)
    // named -> Struct{x: usize, b: usize}
    if is_named {
        quote! {
            #(#field_defs)*
            Some(Self {#(#names),*})
        }
    } else {
        quote! {
            #(#field_defs)*
            Some(Self(#(#names),*))
        }
    }
}

fn construct_generate_function_enum(
    fields: &[GrammarField],
    is_named: bool,
    root_name: &Ident,
    variant_name: &Ident,
) -> proc_macro2::TokenStream {
    if !fields.is_empty() {
        let field_defs = get_field_defs(fields);
        let names = fields.iter().map(|field| &field.name);
        // if the enum variant is
        // non named -> Enum::Variant(x, y, z)
        // named -> Enum::Variant{x: usize, b: usize}
        if is_named {
            quote! {
                #(#field_defs)*
                Some(#root_name::#variant_name {#(#names),*})
            }
        } else {
            quote! {
                #(#field_defs)*
                Some(#root_name::#variant_name (#(#names),*))
            }
        }
    } else {
        // if the num has no fields -> Enum::Variant
        quote! {Some(#root_name::#variant_name {})}
    }
}

struct GrammarField {
    name: Ident,
    id: usize,
    ty: Type,
    attrs: Vec<Attribute>,
    no_mutate: bool,
}

impl GrammarField {
    /// If we have an unnamed tuple or struct, we need to refer to the field as an index instead of
    /// a literal.
    /// Eg: self.0, self.1 instead of self.field, self.field_two
    /// So we need a function since Ident and Index are different syn types.
    /// it's not ideal, but what to do.
    fn get_name(&self, is_named: bool) -> proc_macro2::TokenStream {
        if is_named {
            let name = &self.name;
            quote! {#name}
        } else {
            let name = Index::from(self.id);
            quote! {#name}
        }
    }
}

fn get_fields(fields: &syn::Fields) -> Option<&syn::punctuated::Punctuated<syn::Field, Comma>> {
    match fields {
        syn::Fields::Unnamed(FieldsUnnamed { ref unnamed, .. }) => Some(unnamed),
        syn::Fields::Named(FieldsNamed {
            brace_token: _,
            ref named,
        }) => Some(named),
        _ => None,
    }
}
