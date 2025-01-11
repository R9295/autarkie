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
                        let len = self.#name.__len();
                        if len == 0 {
                            vector.push((::thesis::serialize(&self.#name), <#ty>::id()));
                        }
                }
            });

            let serialized_recursive = parsed.iter().map(|field| {
                let name = field.get_name(is_named);
                quote! {
                    if let Some(fields) = self.#name.serialized() {
                        vector.extend(fields);
                    }
                }
            });

            let register_field = parsed.iter().map(|field| {
                let id = &field.id;
                let ty = &field.ty;
                let name = field.get_name(is_named);
                quote! {
                    let len = self.#name.__len();
                    if len > 0 {
                        v.register_field(((#id, thesis::NodeType::Iterable(len.saturating_sub(1), <#ty>::inner_id().expect("TqeQSVOb____"))), <#ty>::id()));
                    } else {
                        v.register_field(((#id, thesis::NodeType::NonRecursive), <#ty>::id()));
                    }
                    self.#name.fields(v, 0);
                    v.pop_field();
                }
            });
            let register_cmps = parsed.iter().map(|field| {
                let id = &field.id;
                let ty = &field.ty;
                let name = field.get_name(is_named);
                quote! {
                    let len = self.#name.__len();
                    if len > 0 {
                        v.register_field(((#id, thesis::NodeType::Iterable(len.saturating_sub(1), <#ty>::inner_id().expect("My3YTxbe____"))), <#ty>::id()));
                    } else if self.#name.is_recursive() {
                        v.register_field(((#id, thesis::NodeType::Recursive), <#ty>::id()));
                    } else {
                        v.register_field(((#id, thesis::NodeType::NonRecursive), <#ty>::id()));
                    }
                    self.#name.cmps(v, 0, val);
                    v.pop_field();
                
                }
            });

            let inner_mutate = parsed.iter().map(|field| {
                let id = &field.id;
                let name = field.get_name(is_named);
                quote! {
                    #id => {
                        self.#name.__mutate(ty, visitor, path);
                    },
                }
            });
            trait_bounds::add(root_name, &mut base_parsed.generics, &base_parsed.data);
            let (impl_generics, ty_generics, where_clause) = base_parsed.generics.split_for_impl();
            // Generate the Node trait implementation for the Struct
            let node_impl = quote! {
                impl #impl_generics ::thesis::Node for #root_name #ty_generics #where_clause {
                    fn generate(v: &mut thesis::Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
                        *cur_depth += 1usize;
                        #generate
                    }


                    fn fields(&self, v: &mut ::thesis::Visitor, index: usize) {
                        #(#register_field)*;
                    }

                    fn cmps(&self, v: &mut ::thesis::Visitor, index: usize, val: (u64, u64)) {
                        #(#register_cmps)*
                    }

                    fn serialized(&self) -> Option<Vec<(Vec<u8>, thesis::tree::Id)>> {
                        let mut vector = ::std::vec![];
                        #(#serialized_ids);*
                        #(#serialized_recursive);*
                        Some(vector)
                    }

                    fn __mutate(&mut self, ty: &mut thesis::MutationType, visitor: &mut thesis::Visitor, mut path: std::collections::VecDeque<usize>) {
                        if let Some(popped) = path.pop_front() {
                            match popped {
                                #(#inner_mutate)*
                                _ => {
                                    unreachable!("____VzKs1CWu0S")
                                }
                            }
                        } else {
                            match ty {
                                thesis::MutationType::Splice(other) => {
                                    *self = thesis::deserialize(other);
                                }
                                thesis::MutationType::GenerateReplace(ref mut bias) => {
                                    *self = Self::generate(visitor, bias, &mut 0);
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
            let mut recursive_variants = vec![];
            let mut non_recursive_variants = vec![];
            let mut are_we_recursive = vec![];

            for (i, variant) in data.variants.iter().enumerate() {
                let variant_name = &variant.ident;
                let attrs = &variant.attrs;
                let fields = parse_fields(get_fields(&variant.fields));
                
                let mut is_recursive = false;
                for attr in attrs {
                    if let Meta::Path(ref list) = attr.meta {
                        // make sure the attribute we are considering is ours.
                        if list.segments.first().unwrap().ident == "recursive" {
                            is_recursive = true;
                        }
                    }
                }

                let is_named = matches!(variant.fields, syn::Fields::Named(_));
                if is_recursive {
                    recursive_variants.push(quote! {#i,});
                } else {
                    non_recursive_variants.push(quote! {#i,});
                }

                are_we_recursive.push(if !fields.is_empty() {
                    if is_named {
                        quote! {#root_name::#variant_name{..} => #is_recursive}
                    } else {
                        quote! {#root_name::#variant_name(..) => #is_recursive}
                    }
                } else {
                    quote! {
                        #root_name::#variant_name => #is_recursive
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
                        quote!{
                            let len = #name.__len();
                            if len > 0 {
                                v.register_field(((#id, thesis::NodeType::Iterable(len.saturating_sub(1), <#ty>::inner_id().expect("droABVpT____"))), <#ty>::id()));
                            } else if #name.is_recursive() {
                                v.register_field(((#id, thesis::NodeType::Recursive), <#ty>::id()));
                            } else {
                                v.register_field(((#id, thesis::NodeType::NonRecursive), <#ty>::id()));
                            }
                            #name.fields(v, #id);
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
                            v.register_field_stack(((#i, thesis::NodeType::NonRecursive), Self::id()));
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

                let fn_cmp = if !fields.is_empty() {
                    let variant_fields_cmp = fields.iter().map(|field| {
                        let name = &field.name;
                        let ty = &field.ty;
                        let id = &field.id;
                        quote!{
                            let len = #name.__len();
                            if len > 0 {
                                v.register_field(((#id, thesis::NodeType::Iterable(len.saturating_sub(1), <#ty>::inner_id().expect("vBvs6bK4____"))), <#ty>::id()));
                            } else {
                                v.register_field(((#id, thesis::NodeType::NonRecursive), <#ty>::id()));
                            }
                            #name.cmps(v, #id, val);
                            v.pop_field();
                        }
                    });
                    let field_names = fields.iter().map(|field| {
                        let name = &field.name;
                        quote! {#name}
                    });
                    // Note: won't work with enums with generics
                    // cause it won't be Self
                    let match_arm = if is_named {
                        quote! {if let #root_name::#variant_name{#(#field_names),*} = self}
                    } else {
                        quote! {if let #root_name::#variant_name(#(#field_names),*) = self}
                    };
                    Some(quote! {
                            #match_arm {
                            v.register_field_stack(((#i, thesis::NodeType::NonRecursive), Self::id()));
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
                                #name.__mutate(ty, visitor, path);
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
                            if let Some(popped) = path.pop_front() {
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
                            let len = #name.__len();
                            if len == 0 {
                                vector.push((::thesis::serialize(&#name), <#ty>::id()));
                            }
                            if let Some(fields) = #name.serialized() {
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

            if non_recursive_variants.is_empty() && !data.variants.is_empty() {
                panic!(
                    "{:?} has no non-recursive variants. This will lead to stack overflows.",
                    root_name
                );
            }

            let generate_func = if data.variants.is_empty() {
                quote! {
                    Self {}
                }
            } else {
                let variant_id_calculation = if !recursive_variants.is_empty() {
                    let recursive_variant_count = recursive_variants
                        .len()
                        .checked_sub(1)
                        .expect("nFeGkMPw____");
                    let non_recursive_variant_count = non_recursive_variants
                        .len()
                        .checked_sub(1)
                        .expect("we must have atleast 1 non-recursive variant");
                    quote! {
                        let r_variants = [#(#recursive_variants)*];
                        let nr_variants = [#(#non_recursive_variants)*];
                        let choose_recursive = *depth > 0usize && v.coinflip() && *cur_depth < 100;
                        let variant_id = if choose_recursive {
                                let index = v.random_range(0usize, #recursive_variant_count);
                                *depth = depth.checked_sub(1).expect("XVldNrja____");
                                r_variants[index]
                        } else {
                            let index = v.random_range(0usize, #non_recursive_variant_count);
                            nr_variants[index]
                        };
                    }
                } else {
                    let variant_count = non_recursive_variants
                        .len()
                        .checked_sub(1)
                        .expect("we must have atleast 1 non-recursive variant");
                    quote! {
                        let variant_id = v.random_range(0usize, #variant_count);
                    }
                };
                quote! {
                        *cur_depth += 1usize;
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
                impl #impl_generics ::thesis::Node for #root_name #ty_generics #where_clause {
                    fn generate(v: &mut ::thesis::Visitor, depth: &mut usize, cur_depth: &mut usize) -> Self {
                        #generate_func
                    }


                    fn fields(&self, v: &mut ::thesis::Visitor, index: usize) {
                        #(#fn_fields)*;
                    }

                    fn cmps(&self, v: &mut ::thesis::Visitor, index: usize, val: (u64, u64)) {
                        #(#fn_cmps)*;
                    }

                    fn serialized(&self) -> Option<Vec<(Vec<u8>, thesis::tree::Id)>> {
                        let mut vector = ::std::vec![];
                        match self {
                             #(#serialized,)*
                        }
                        Some(vector)
                    }

                    fn is_recursive(&self) -> bool {
                        match self {
                            #(#are_we_recursive,)*
                        }
                    }

                    fn __mutate(&mut self, ty: &mut thesis::MutationType, visitor: &mut thesis::Visitor, mut path: std::collections::VecDeque<usize>) {
                        if let Some(popped) = path.pop_front() {
                            match popped {
                                #(#inner_mutate)*
                                _ => unreachable!("____VpyAL0wN7m")
                            }
                        }
                        else {
                            match ty {
                                thesis::MutationType::Splice(other) => {
                                    *self = thesis::deserialize(other);
                                }
                                thesis::MutationType::GenerateReplace(ref mut bias) => {
                                    *self = Self::generate(visitor, bias, &mut 0);
                                }
                                thesis::MutationType::RecursiveReplace => {
                                    if self.is_recursive() {
                                        // 0 depth == always non-recursive
                                        *self = Self::generate(visitor, &mut 0, &mut 0);
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
    fields
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
        .collect::<Vec<_>>()
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
                    let #name = <#ty>::generate(v, depth, cur_depth);
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
            Self {#(#names),*}
        }
    } else {
        quote! {
            #(#field_defs)*
            Self(#(#names),*)
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
                #root_name::#variant_name {#(#names),*}
            }
        } else {
            quote! {
                #(#field_defs)*
                #root_name::#variant_name (#(#names),*)
            }
        }
    } else {
        // if the num has no fields -> Enum::Variant
        quote! {#root_name::#variant_name {}}
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
