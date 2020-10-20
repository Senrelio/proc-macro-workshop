extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput, Type};

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = ast.ident;
    let b_name = format_ident!("{}Builder", name);
    let fields = if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(syn::FieldsNamed { named: fields, .. }),
        ..
    }) = ast.data
    {
        fields
    } else {
        unimplemented!("builder proc-macro can only apply to struct.");
    };

    let b_fields = fields.iter().map(|f| {
        let f_name = &f.ident;
        let ty = &f.ty;
        match is_option(ty).is_some() || is_vec(ty).is_some() {
            true => {
                quote! {
                    #f_name: #ty
                }
            }
            false => {
                quote! {
                    #f_name: Option<#ty>
                }
            }
        }
    });

    let b_init = fields.iter().map(|f| {
        let f_name = &f.ident;
        let ty = &f.ty;
        match is_vec(ty).is_some() {
            true => quote! {
                #f_name: Vec::new()
            },
            false => quote! {
                #f_name: None
            },
        }
    });

    let extract = fields.iter().map(|f| {
        let f_name = &f.ident;
        let ty = &f.ty;
        let is_option = is_option(ty).is_some();
        let is_vec = is_vec(ty).is_some();
        match (is_option, is_vec) {
            (true, false) => quote! {
                #f_name: self.#f_name.clone()
            },
            (true, true) => unimplemented!(),
            (false, false) => quote! {
                #f_name: self.#f_name.clone().ok_or("field cannot be none")?
            },
            (false, true) => quote! {
                #f_name: self.#f_name.clone()
            },
        }
    });

    let b_methods = fields.iter().map(|f| {
        let f_name = &f.ident;
        let ty = &f.ty;
        let is_option = is_option(ty);
        let is_vec = is_vec(ty);
        match (is_option, is_vec) {
            (Some(t), None) => quote! {
                pub fn #f_name(&mut self, #f_name: #t) -> &mut Self {
                    self.#f_name = Some(#f_name);
                    self
                }
            },
            (Some(_), Some(_)) => unimplemented!(),
            (None, None) => quote! {
                pub fn #f_name(&mut self, #f_name: #ty) -> &mut Self {
                    self.#f_name = Some(#f_name);
                    self
                }
            },
            (None, Some(_)) => quote! {
                pub fn #f_name(&mut self, #f_name: #ty) -> &mut Self {
                    self.#f_name = #f_name;
                    self
                }
            },
        }
    });

    let expand = quote! {
        impl #name {
            pub fn builder() -> #b_name {
                #b_name {
                    #(
                        #b_init
                    ),*
                }
            }
        }
        pub struct #b_name {
            #(
                #b_fields
            ),*
        }
        impl #b_name {
            pub fn build(&self) -> Result<#name, String> {
                Ok(#name {
                    #(
                        #extract
                    ),*
                })
            }
            #(
                #b_methods
            )*
        }
    };
    expand.into()
}

fn is_option(ty: &Type) -> Option<&Type> {
    if let Type::Path(syn::TypePath { path, .. }) = ty {
        assert_eq!(
            path.segments.len(),
            1,
            "cannot handle type with more than 1 segment"
        );
        let seg = &path.segments[0];
        if seg.ident != "Option" {
            return None;
        }
        let args = &seg.arguments;
        if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
            args: inside_bracket,
            ..
        }) = args
        {
            assert_eq!(
                inside_bracket.len(),
                1,
                "cannot have more than one generic type"
            );
            if let syn::GenericArgument::Type(inner_type) = &inside_bracket[0] {
                Some(inner_type)
            } else {
                unimplemented!()
            }
        } else {
            None
        }
    } else {
        unimplemented!()
    }
}

fn is_vec(ty: &Type) -> Option<&Type> {
    if let Type::Path(syn::TypePath { path, .. }) = ty {
        assert_eq!(
            path.segments.len(),
            1,
            "cannot handle type with more than 1 segment"
        );
        let seg = &path.segments[0];
        if seg.ident != "Vec" {
            return None;
        }
        let args = &seg.arguments;
        if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
            args: inside_bracket,
            ..
        }) = args
        {
            assert_eq!(
                inside_bracket.len(),
                1,
                "cannot have more than one generic type"
            );
            if let syn::GenericArgument::Type(inner_type) = &inside_bracket[0] {
                Some(inner_type)
            } else {
                unimplemented!()
            }
        } else {
            None
        }
    } else {
        unimplemented!()
    }
}

// struct User {
//     id: String,
// }

// impl User {
//     fn builder() -> UserBuilder {
//         UserBuilder { id: None }
//     }
// }

// struct UserBuilder {
//     id: Option<String>,
// }

// impl UserBuilder {
//     fn id(&mut self, id: String) -> &mut Self {
//         self.id = Some(id);
//         self
//     }
//     fn build(&self) -> User {
//         User {
//             id: self.id.clone().unwrap(),
//         }
//     }
// }

// #[test]
// fn dummy() {
//     let user = User::builder().id("song".to_owned()).build();
// }
