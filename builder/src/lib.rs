extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput, Type};

#[proc_macro_derive(Builder, attributes(builder))]
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
                    #f_name: std::option::Option<#ty>
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
                    self.#f_name = std::option::Option::Some(#f_name);
                    self
                }
            },
            (Some(_), Some(_)) => unimplemented!(),
            (None, None) => quote! {
                pub fn #f_name(&mut self, #f_name: #ty) -> &mut Self {
                    self.#f_name = std::option::Option::Some(#f_name);
                    self
                }
            },
            (None, Some(_)) => {
                let attrs = &f.attrs;
                {
                    let need_extend = attrs.len() == 1
                        && attrs[0].path.segments.len() == 1
                        && attrs[0].path.segments[0].ident == "builder";
                    if need_extend {
                        let each_append = handle_builder_attribute(&attrs[0], f);
                        if let Ok(a) = each_append {
                            quote! {
                                #a
                            }
                        } else {
                            each_append.unwrap_err().to_compile_error()
                        }
                    } else {
                        let common = quote! {
                            pub fn #f_name(&mut self, #f_name: #ty) -> &mut Self {
                                self.#f_name = #f_name;
                                self
                            }
                        };
                        quote! {
                            #common
                        }
                    }
                }
            }
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
            pub fn build(&self) -> std::result::Result<#name, String> {
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

fn handle_builder_attribute(
    attr: &syn::Attribute,
    f: &syn::Field,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    let name = &f.ident;
    let ty = &f.ty;
    // assert!(is_vec(ty).is_some(), "builder attribute has to be applied to vec type fields");
    let inner_ty = is_vec(ty).ok_or_else(|| syn::Error::new(
        name.clone().unwrap().span(),
        "expected `each`",
    ))?;
    let meta_list = if let syn::Meta::List(meta_list) = attr.parse_meta().ok().ok_or_else(||
        syn::Error::new(name.clone().unwrap().span(), "expected `each`"),
    )? {
        meta_list
    } else {
        unimplemented!()
    };
    assert_eq!(meta_list.path.segments.len(), 1);
    assert_eq!(meta_list.path.segments[0].ident, "builder");
    assert_eq!(meta_list.nested.len(), 1);
    let kv_pair = if let syn::NestedMeta::Meta(syn::Meta::NameValue(kv_pair)) = &meta_list.nested[0]
    {
        if kv_pair.path.segments[0].ident != "each" {
            return Err(syn::Error::new_spanned(
                meta_list,
                "expected `builder(each = \"...\")`",
            ));
        }
        kv_pair
    } else {
        unimplemented!()
    };
    let arg = if let syn::Lit::Str(arg) = &kv_pair.lit {
        arg
    } else {
        unimplemented!()
    };
    let arg = syn::Ident::new(&arg.value(), arg.span());
    Ok(quote! {
        fn #arg(&mut self, #arg: #inner_ty) -> &mut Self {
            self.#name.push(#arg);
            self
        }
    })
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

#[proc_macro_derive(HelperAttr, attributes(builder))]
pub fn builder_helper_attr(_item: TokenStream) -> TokenStream {
    TokenStream::new()
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
