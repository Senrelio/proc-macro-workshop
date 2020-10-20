extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DataStruct, DeriveInput, Fields, FieldsNamed};

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = ast.ident;
    let b_name = format_ident!("{}Builder", name);

    let expand = quote! {
        impl #name {
            fn builder() -> #b_name {
                #b_name {
                    #(
                        #b_init
                    ),*
                }
            }
        }
        struct #b_name {
            #(
                #b_fields
            ),*
        }
        impl #b_name {
            fn build(self) -> #name {
                #name {
                    #(
                        #extract
                    ),*
                }
            }
            #(
                #b_methods
            )*
        }
    };
    expand.into()
}

