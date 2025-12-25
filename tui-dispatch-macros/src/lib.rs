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
                impl tui_dispatch::Action for #name {
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

/// Derive macro for the BindingContext trait
///
/// Generates implementations for `name()`, `from_name()`, and `all()` methods.
/// The context name is derived from the variant name converted to snake_case.
///
/// # Example
/// ```ignore
/// #[derive(BindingContext, Clone, Copy, PartialEq, Eq, Hash)]
/// enum MyContext {
///     Default,
///     Search,
///     ConnectionForm,
/// }
///
/// // Generated names: "default", "search", "connection_form"
/// assert_eq!(MyContext::Default.name(), "default");
/// assert_eq!(MyContext::from_name("search"), Some(MyContext::Search));
/// ```
#[proc_macro_derive(BindingContext)]
pub fn derive_binding_context(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = match &input.data {
        syn::Data::Enum(data) => {
            // Check that all variants are unit variants
            for variant in &data.variants {
                if !matches!(variant.fields, syn::Fields::Unit) {
                    return syn::Error::new_spanned(
                        variant,
                        "BindingContext can only be derived for enums with unit variants",
                    )
                    .to_compile_error()
                    .into();
                }
            }

            let variant_names: Vec<_> = data.variants.iter().map(|v| &v.ident).collect();
            let variant_strings: Vec<_> = variant_names
                .iter()
                .map(|v| to_snake_case(&v.to_string()))
                .collect();

            let name_arms = variant_names.iter().zip(variant_strings.iter()).map(|(v, s)| {
                quote! { #name::#v => #s }
            });

            let from_name_arms = variant_names.iter().zip(variant_strings.iter()).map(|(v, s)| {
                quote! { #s => ::core::option::Option::Some(#name::#v) }
            });

            let all_variants = variant_names.iter().map(|v| quote! { #name::#v });

            quote! {
                impl tui_dispatch::BindingContext for #name {
                    fn name(&self) -> &'static str {
                        match self {
                            #(#name_arms),*
                        }
                    }

                    fn from_name(name: &str) -> ::core::option::Option<Self> {
                        match name {
                            #(#from_name_arms,)*
                            _ => ::core::option::Option::None,
                        }
                    }

                    fn all() -> &'static [Self] {
                        static ALL: &[#name] = &[#(#all_variants),*];
                        ALL
                    }
                }
            }
        }
        _ => {
            return syn::Error::new_spanned(input, "BindingContext can only be derived for enums")
                .to_compile_error()
                .into();
        }
    };

    TokenStream::from(expanded)
}

/// Derive macro for the ComponentId trait
///
/// Generates implementations for `name()` method that returns the variant name.
///
/// # Example
/// ```ignore
/// #[derive(ComponentId, Clone, Copy, PartialEq, Eq, Hash, Debug)]
/// enum MyComponentId {
///     Sidebar,
///     MainContent,
///     StatusBar,
/// }
///
/// assert_eq!(MyComponentId::Sidebar.name(), "Sidebar");
/// ```
#[proc_macro_derive(ComponentId)]
pub fn derive_component_id(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = match &input.data {
        syn::Data::Enum(data) => {
            // Check that all variants are unit variants
            for variant in &data.variants {
                if !matches!(variant.fields, syn::Fields::Unit) {
                    return syn::Error::new_spanned(
                        variant,
                        "ComponentId can only be derived for enums with unit variants",
                    )
                    .to_compile_error()
                    .into();
                }
            }

            let variant_names: Vec<_> = data.variants.iter().map(|v| &v.ident).collect();
            let variant_strings: Vec<_> = variant_names
                .iter()
                .map(|v| v.to_string())
                .collect();

            let name_arms = variant_names.iter().zip(variant_strings.iter()).map(|(v, s)| {
                quote! { #name::#v => #s }
            });

            quote! {
                impl tui_dispatch::ComponentId for #name {
                    fn name(&self) -> &'static str {
                        match self {
                            #(#name_arms),*
                        }
                    }
                }
            }
        }
        _ => {
            return syn::Error::new_spanned(input, "ComponentId can only be derived for enums")
                .to_compile_error()
                .into();
        }
    };

    TokenStream::from(expanded)
}

/// Convert a PascalCase string to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }
    result
}
