//! Procedural macros for tui-dispatch

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derive macro for the Action trait
///
/// Generates a `name()` method that returns the variant name as a static string.
///
/// # Example
/// ```ignore
/// #[derive(Action, Clone, Debug)]
/// enum MyAction {
///     SelectItem(usize),
///     LoadData,
/// }
///
/// let action = MyAction::SelectItem(0);
/// assert_eq!(action.name(), "SelectItem");
/// ```
#[proc_macro_derive(Action)]
pub fn derive_action(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = match &input.data {
        syn::Data::Enum(data) => {
            let variants = data.variants.iter().map(|v| {
                let variant_name = &v.ident;
                let variant_str = variant_name.to_string();

                match &v.fields {
                    syn::Fields::Unit => quote! {
                        #name::#variant_name => #variant_str
                    },
                    syn::Fields::Unnamed(_) => quote! {
                        #name::#variant_name(..) => #variant_str
                    },
                    syn::Fields::Named(_) => quote! {
                        #name::#variant_name { .. } => #variant_str
                    },
                }
            });

            quote! {
                impl tui_dispatch_core::Action for #name {
                    fn name(&self) -> &'static str {
                        match self {
                            #(#variants),*
                        }
                    }
                }
            }
        }
        _ => {
            return syn::Error::new_spanned(input, "Action can only be derived for enums")
                .to_compile_error()
                .into();
        }
    };

    TokenStream::from(expanded)
}
