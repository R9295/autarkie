extern crate proc_macro2;

use proc_macro::TokenStream;
use quote::quote;
mod trait_bounds;
mod utils;
use syn::{spanned::Spanned, token::Comma, *};

#[proc_macro_derive(Grammar, attributes(literal, recursive))]
pub fn derive_node(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut base_parsed = syn::parse_macro_input!(input as syn::DeriveInput);
    let root_name = &base_parsed.ident;
    let expanded = match base_parsed.data {
        Data::Struct(ref data) => {
            let fields = get_fields(&data.fields);
            let is_named = matches!(data.fields, syn::Fields::Named(_));
            let parsed = parse_fields(fields);
            let generate = construct_generate_function_struct(&parsed, is_named);

            let serialized_ids = parsed.iter().map(|field| {
                let name = field.get_name(is_named);
                let ty = &field.ty;
                quote! {
                        if !self.#name.__autarkie_node_ty(autarkie_visitor).is_iterable() {
                            vector.push((::autarkie::serialize(&self.#name), <#ty>::__autarkie_id()));
                        }
                }
            });

            let serialized_recursive = parsed.iter().map(|field| {
                let name = field.get_name(is_named);
                quote! {
                    if let Some(fields) = self.#name.__autarkie_serialized(autarkie_visitor) {
                        vector.extend(fields);
                    }
                }
            });

            let register_field = parsed.iter().map(|field| {
                let id = &field.id;
                let ty = &field.ty;
                let name = field.get_name(is_named);
                quote! {
                    v.register_field(((#id, self.#name.__autarkie_node_ty(v)), <#ty>::__autarkie_id()));
                    self.#name.__autarkie_fields(v, 0);
                    v.pop_field();
                }
            });
            let register_cmps = parsed.iter().map(|field| {
                let id = &field.id;
                let ty = &field.ty;
                let name = field.get_name(is_named);
                quote! {
                    v.register_field(((#id, self.#name.__autarkie_node_ty(v)), <#ty>::__autarkie_id()));
                    self.#name.__autarkie_cmps(v, 0, val);
                    v.pop_field();

                }
            });

            let register_ty = parsed.iter().map(|field| {
                let ty = &field.ty;
                quote! {
                    if !v.is_recursive(<#ty>::__autarkie_id()) {
                        <#ty>::__autarkie_register(v, Some(Self::__autarkie_id()), 0);
                    } else {
                        v.register_ty(Some(Self::__autarkie_id()), <#ty>::__autarkie_id(), 0);
                        v.pop_ty();
                    }
                }
            });

            let inner_mutate = parsed.iter().map(|field| {
                let id = &field.id;
                let name = field.get_name(is_named);
                quote! {
                    #id => {
                        self.#name.__autarkie_mutate(autarkie_ty, autarkie_visitor, autarkie_path);
                    },
                }
            });
            trait_bounds::add(root_name, &mut base_parsed.generics, &base_parsed.data);
            let (impl_generics, ty_generics, where_clause) = base_parsed.generics.split_for_impl();
            // Generate the Node trait implementation for the Struct
            let node_impl = quote! {
                impl #impl_generics ::autarkie::Node for #root_name #ty_generics #where_clause {
                    fn __autarkie_generate(v: &mut autarkie::Visitor, depth: &mut usize, cur_depth: &mut usize) -> Option<Self> {
                        #generate
                    }

                    fn __autarkie_register(v: &mut ::autarkie::Visitor, parent: Option<::autarkie::tree::Id>, variant: usize) {
                        v.register_ty(parent, Self::__autarkie_id(), variant);
                        #(#register_ty)*;
                        v.pop_ty();
                    }

                    fn __autarkie_fields(&self, v: &mut ::autarkie::Visitor, index: usize) {
                        #(#register_field)*;
                    }


                    fn __autarkie_cmps(&self, v: &mut ::autarkie::Visitor, index: usize, val: (u64, u64)) {
                        #(#register_cmps)*
                    }

                    fn __autarkie_serialized(&self, autarkie_visitor: &::autarkie::Visitor) -> Option<std::vec::Vec<(std::vec::Vec<u8>, autarkie::tree::Id)>> {
                        let mut vector = ::std::vec![];
                        #(#serialized_ids);*
                        #(#serialized_recursive);*
                        Some(vector)
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
                                    if let Some(generated) = Self::__autarkie_generate(autarkie_visitor, bias, &mut 0) {
                                        *self = generated;
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
            let mut serialized = vec![];
            let mut fn_cmps = vec![];
            let mut are_we_recursive = vec![];
            let mut register_ty = vec![];

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

                let field_fn = if !fields.is_empty() {
                    let variant_fields_register = fields.iter().map(|field| {
                        let name = &field.name;
                        let ty = &field.ty;
                        let id = &field.id;
                        quote! {
                            v.register_field(((#id, #name.__autarkie_node_ty(v)), <#ty>::__autarkie_id()));
                            #name.__autarkie_fields(v, #id);
                            v.pop_field();
                        }
                    });
                    let field_names = fields.iter().map(|field| {
                        let name = &field.name;
                        quote! {#name}
                    });
                    let match_arm = if is_named {
                        quote! {if let #root_name::#variant_name{#(#field_names),*} = self}
                    } else {
                        quote! {if let #root_name::#variant_name(#(#field_names),*) = self}
                    };
                    Some(quote! {
                            #match_arm {
                            v.register_field_stack(((#i, self.__autarkie_node_ty(v)), Self::__autarkie_id()));
                            #(#variant_fields_register)*
                            v.pop_field();
                        }
                    })
                } else {
                    Some(quote! {
                        if let #root_name::#variant_name{} = self {}
                    })
                };

                fn_fields.push(field_fn);
                if fields.is_empty() {
                register_ty.push(quote! {
                    // use something besides bool; bool is just a place holder.
                    v.register_ty(Some(Self::__autarkie_id()), <std::marker::PhantomData<bool>>::__autarkie_id(), #i);
                    v.pop_ty();
                });
                } else {
                    let field_names = fields.iter().map(|field| {
                        let ty = &field.ty;
                        quote! {
                            if !v.is_recursive(<#ty>::__autarkie_id()) {
                                <#ty>::__autarkie_register(v, Some(Self::__autarkie_id()), #i);
                            } else {
                                v.register_ty(Some(Self::__autarkie_id()), <#ty>::__autarkie_id(), #i);
                                v.pop_ty();
                            }
                        }
                    });
                    register_ty.push(quote! {#(#field_names)*});
                }

                let fn_cmp = if !fields.is_empty() {
                    let variant_fields_cmp = fields.iter().map(|field| {
                        let name = &field.name;
                        let ty = &field.ty;
                        let id = &field.id;
                        quote! {
                            v.register_field(((#id, #name.__autarkie_node_ty(v)), <#ty>::__autarkie_id()));
                            #name.__autarkie_cmps(v, #id, val);
                            v.pop_field();
                        }
                    });
                    let field_names = fields.iter().map(|field| {
                        let name = &field.name;
                        quote! {#name}
                    });
                    let match_arm = if is_named {
                        quote! {if let #root_name::#variant_name{#(#field_names),*} = self}
                    } else {
                        quote! {if let #root_name::#variant_name(#(#field_names),*) = self}
                    };
                    Some(quote! {
                            #match_arm {
                            v.register_field_stack(((#i, self.__autarkie_node_ty(v)), Self::__autarkie_id()));
                            #(#variant_fields_cmp)*
                            v.pop_field();
                        }
                    })
                } else {
                    Some(quote! {
                        if let #root_name::#variant_name{} = self {}
                    })
                };

                fn_cmps.push(fn_cmp);
                let inner_mutate_variant = if !fields.is_empty() {
                    let field_names = fields.iter().map(|field| {
                        let name = &field.name;
                        quote! {#name}
                    });
                    let variant_fields_mutate = fields.iter().map(|field| {
                        let name = &field.name;
                        let id = &field.id;
                        quote! {
                            #id => {
                                #name.__autarkie_mutate(autarkie_ty, autarkie_visitor, autarkie_path);
                            },
                        }
                    });

                    let match_arm = if is_named {
                        quote! {if let #root_name::#variant_name{#(#field_names),*} = self }
                    } else {
                        quote! {if let #root_name::#variant_name(#(#field_names),*) = self }
                    };

                    Some(quote! {
                        #i => {
                         #match_arm {
                            if let Some(popped) = autarkie_path.pop_front() {
                             match popped {
                                 #(#variant_fields_mutate)*
                                 _ => {
                                     unreachable!("____FU1zlV0c")
                                 }
                             }
                            } else {
                                unreachable!("____kTHVIHpB");
                            }
                         }
                        },
                    })
                } else {
                    Some(quote! {
                        #i => unreachable!("____aNHh8Ap8"),
                    })
                };

                inner_mutate.push(inner_mutate_variant);

                if !fields.is_empty() {
                    let field_names = fields.iter().map(|field| {
                        let name = &field.name;
                        quote! {#name}
                    });
                    let match_arm = if is_named {
                        quote! {Self::#variant_name{#(#field_names),*} => }
                    } else {
                        quote! {Self::#variant_name(#(#field_names),*) => }
                    };
                    let serialized_fields = fields.iter().map(|field| {
                        let name = &field.name;
                        let ty = &field.ty;
                        quote! {
                        if !#name.__autarkie_node_ty(autarkie_visitor).is_iterable() {
                                vector.push((::autarkie::serialize(&#name), <#ty>::__autarkie_id()));
                            }
                            if let Some(fields) = #name.__autarkie_serialized(autarkie_visitor) {
                                vector.extend(fields);
                            }
                        }
                    });
                    let serialized_variant = quote! {
                    #match_arm {
                        #(#serialized_fields)*
                    }
                    };
                    serialized.push(serialized_variant);
                } else {
                    serialized.push(quote! {
                        Self::#variant_name{} => {
                        }
                    })
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
                        if is_recursive {
                            *cur_depth += 1usize;
                        }
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
                    fn __autarkie_generate(v: &mut ::autarkie::Visitor, depth: &mut usize, cur_depth: &mut usize) -> Option<Self> {
                        #generate_func
                    }

                    fn __autarkie_fields(&self, v: &mut ::autarkie::Visitor, index: usize) {
                        #(#fn_fields)*;
                    }

                    fn __autarkie_register(v: &mut ::autarkie::Visitor, parent: Option<::autarkie::tree::Id>, variant: usize) {
                        v.register_ty(parent, Self::__autarkie_id(), variant);
                        #(#register_ty)*;
                        v.pop_ty();
                    }

                    fn __autarkie_cmps(&self, v: &mut ::autarkie::Visitor, index: usize, val: (u64, u64)) {
                        #(#fn_cmps)*;
                    }

                    fn __autarkie_serialized(&self, autarkie_visitor: &::autarkie::Visitor) -> Option<std::vec::Vec<(std::vec::Vec<u8>, autarkie::tree::Id)>> {
                        let mut vector = ::std::vec![];
                        match self {
                             #(#serialized,)*
                        }
                        Some(vector)
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
                                    if let Some(generated) = Self::__autarkie_generate(autarkie_visitor, bias, &mut 0) {
                                        *self = generated;
                                    }
                                }
                                autarkie::MutationType::RecursiveReplace => {
                                    if self.__autarkie_node_ty(autarkie_visitor).is_recursive() {
                                        // 0 depth == always non-recursive
                                    if let Some(generated) = Self::__autarkie_generate(autarkie_visitor, &mut 0, &mut 0) {
                                        *self = generated;
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
        Data::Union(..) => todo!(),
    };
    TokenStream::from(expanded)
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
                None => Ident::new(&format!("_{}", id), field.span()),
            };
            GrammarField {
                name,
                ty: ty.clone(),
                id,
                attrs: field.attrs.clone(),
            }
        })
        .collect::<Vec<_>>();
    fields
}

fn get_field_defs(fields: &Vec<GrammarField>) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|field| {
            let attr_iterator = field.attrs.iter();
            let name = &field.name;
            let ty = &field.ty;
            let mut generator = None;
            // The generator is a closure that is run immediately.
            // This allows us to sepcify literals for a field.
            // TODO: maybe do some sanitization of literals
            for attr in attr_iterator {
                if let Meta::List(ref list) = attr.meta {
                    // make sure the attribute we are considering is ours.
                    if list.path.segments.first().unwrap().ident == "literal" {
                        let literals = list
                            .tokens
                            .clone()
                            .into_iter()
                            .filter(|i| {
                                matches!(i, proc_macro2::TokenTree::Literal(_))
                                    || matches!(i, proc_macro2::TokenTree::Group(_))
                                    || matches!(i, proc_macro2::TokenTree::Ident(_))
                            })
                            .collect::<Vec<_>>();
                        let literals_len = literals.len() - 1;
                        // if we only have one literal
                        if literals_len == 0 {
                            let item = literals.first().unwrap();
                            generator = Some(quote! {
                                let #name = #item as #ty;
                            });
                        } else {
                            // if we have multiple literals -> pick one randomly
                            generator = Some(quote! {
                                let #name = || -> #ty {
                                    let item = v.random_range(0, #literals_len);
                                    let literals = [#(#literals),*];
                                    literals[item] as #ty
                                }();
                            });
                        }
                    }
                }
            }
            // If we do not have a literal attribute, we use the inner generate function of the type.
            if generator.is_none() {
                generator = Some(quote! {
                    let #name = <#ty>::__autarkie_generate(v, depth, cur_depth)?;
                });
            }
            // this should never happen, cause we either have a literal attribute or not.
            generator
                .unwrap_or_else(|| panic!("invariant; field {:?} did not have a generator", name))
        })
        .collect::<Vec<_>>()
}

fn construct_generate_function_struct(
    fields: &Vec<GrammarField>,
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
    fields: &Vec<GrammarField>,
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
