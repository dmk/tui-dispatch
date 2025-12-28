//! Procedural macros for tui-dispatch

use darling::{FromDeriveInput, FromVariant};
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{parse_macro_input, DeriveInput};

/// Container-level attributes for #[derive(Action)]
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(action), supports(enum_any))]
struct ActionOpts {
    ident: syn::Ident,
    data: darling::ast::Data<ActionVariant, ()>,

    /// Enable automatic category inference from variant name prefixes
    #[darling(default)]
    infer_categories: bool,

    /// Generate dispatcher trait
    #[darling(default)]
    generate_dispatcher: bool,
}

/// Variant-level attributes
#[derive(Debug, FromVariant)]
#[darling(attributes(action))]
struct ActionVariant {
    ident: syn::Ident,
    fields: darling::ast::Fields<()>,

    /// Explicit category override
    #[darling(default)]
    category: Option<String>,

    /// Exclude from category inference
    #[darling(default)]
    skip_category: bool,
}

/// Common action verbs that typically appear as the last part of a variant name
// Action verbs that typically END an action name (the actual verb part)
// Things like "Form", "Panel", "Field" are nouns, not verbs - they should NOT be here
const ACTION_VERBS: &[&str] = &[
    // State transitions
    "Start", "End", "Open", "Close", "Submit", "Confirm", "Cancel", // Navigation
    "Next", "Prev", "Up", "Down", "Left", "Right", "Enter", "Exit", "Escape",
    // CRUD operations
    "Add", "Remove", "Clear", "Update", "Set", "Get", "Load", "Save", "Delete", "Create",
    // Visibility
    "Show", "Hide", "Enable", "Disable", "Toggle", // Focus
    "Focus", "Blur", "Select", // Movement
    "Move", "Copy", "Cycle", "Reset", "Scroll",
];

/// Split a PascalCase string into parts
fn split_pascal_case(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();

    for ch in s.chars() {
        if ch.is_uppercase() && !current.is_empty() {
            parts.push(current);
            current = String::new();
        }
        current.push(ch);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// Convert PascalCase to snake_case
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

/// Convert snake_case to PascalCase
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

/// Infer category from a variant name using naming patterns
fn infer_category(name: &str) -> Option<String> {
    let parts = split_pascal_case(name);
    if parts.is_empty() {
        return None;
    }

    // Check for "Did" prefix (async results)
    if parts[0] == "Did" {
        return Some("async_result".to_string());
    }

    // If only one part, no category
    if parts.len() < 2 {
        return None;
    }

    // Find the longest prefix that ends before an action verb
    // e.g., ["Connection", "Form", "Submit"] -> "connection_form"
    // e.g., ["Search", "Add", "Char"] -> "search"
    // e.g., ["Value", "Viewer", "Scroll", "Up"] -> "value_viewer"

    let first_is_verb = ACTION_VERBS.contains(&parts[0].as_str());

    let mut prefix_end = parts.len();
    let mut found_verb = false;
    for (i, part) in parts.iter().enumerate().skip(1) {
        if ACTION_VERBS.contains(&part.as_str()) {
            prefix_end = i;
            found_verb = true;
            break;
        }
    }

    // Skip if first part is an action verb - these are primary actions, not categorized
    // e.g., "OpenConnectionForm" → "Open" is the verb, "ConnectionForm" is the object
    // e.g., "NextItem" → "Next" is the verb, "Item" is the object
    if first_is_verb {
        return None;
    }

    // Skip if no verb found in the name - can't determine meaningful category
    if !found_verb {
        return None;
    }

    if prefix_end == 0 {
        return None;
    }

    let prefix_parts: Vec<&str> = parts[..prefix_end].iter().map(|s| s.as_str()).collect();
    let prefix = prefix_parts.join("");

    Some(to_snake_case(&prefix))
}

/// Derive macro for the Action trait
///
/// Generates a `name()` method that returns the variant name as a static string.
///
/// With `#[action(infer_categories)]`, also generates:
/// - `category() -> Option<&'static str>` - Get action's category
/// - `category_enum() -> {Name}Category` - Get category as enum
/// - `is_{category}()` predicates for each category
/// - `{Name}Category` enum with all discovered categories
///
/// With `#[action(generate_dispatcher)]`, also generates:
/// - `{Name}Dispatcher` trait with category-based dispatch methods
///
/// # Example
/// ```ignore
/// #[derive(Action, Clone, Debug)]
/// #[action(infer_categories, generate_dispatcher)]
/// enum MyAction {
///     SearchStart,
///     SearchClear,
///     ConnectionFormOpen,
///     ConnectionFormSubmit,
///     DidConnect,
///     Tick,  // uncategorized
/// }
///
/// let action = MyAction::SearchStart;
/// assert_eq!(action.name(), "SearchStart");
/// assert_eq!(action.category(), Some("search"));
/// assert!(action.is_search());
/// ```
#[proc_macro_derive(Action, attributes(action))]
pub fn derive_action(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Try to parse with darling for attributes
    let opts = match ActionOpts::from_derive_input(&input) {
        Ok(opts) => opts,
        Err(e) => return e.write_errors().into(),
    };

    let name = &opts.ident;

    let variants = match &opts.data {
        darling::ast::Data::Enum(variants) => variants,
        _ => {
            return syn::Error::new_spanned(&input, "Action can only be derived for enums")
                .to_compile_error()
                .into();
        }
    };

    // Generate basic name() implementation
    let name_arms = variants.iter().map(|v| {
        let variant_name = &v.ident;
        let variant_str = variant_name.to_string();

        match &v.fields.style {
            darling::ast::Style::Unit => quote! {
                #name::#variant_name => #variant_str
            },
            darling::ast::Style::Tuple => quote! {
                #name::#variant_name(..) => #variant_str
            },
            darling::ast::Style::Struct => quote! {
                #name::#variant_name { .. } => #variant_str
            },
        }
    });

    let mut expanded = quote! {
        impl tui_dispatch::Action for #name {
            fn name(&self) -> &'static str {
                match self {
                    #(#name_arms),*
                }
            }
        }
    };

    // If category inference is enabled, generate category-related code
    if opts.infer_categories {
        // Collect categories and their variants
        let mut categories: HashMap<String, Vec<&Ident>> = HashMap::new();
        let mut variant_categories: Vec<(&Ident, Option<String>)> = Vec::new();

        for v in variants.iter() {
            let cat = if v.skip_category {
                None
            } else if let Some(ref explicit_cat) = v.category {
                Some(explicit_cat.clone())
            } else {
                infer_category(&v.ident.to_string())
            };

            variant_categories.push((&v.ident, cat.clone()));

            if let Some(ref category) = cat {
                categories
                    .entry(category.clone())
                    .or_default()
                    .push(&v.ident);
            }
        }

        // Sort categories for deterministic output
        let mut sorted_categories: Vec<_> = categories.keys().cloned().collect();
        sorted_categories.sort();

        // Create deduplicated category match arms
        let category_arms_dedup: Vec<_> = variant_categories
            .iter()
            .map(|(variant, cat)| {
                let cat_expr = match cat {
                    Some(c) => quote! { ::core::option::Option::Some(#c) },
                    None => quote! { ::core::option::Option::None },
                };
                // Use wildcard pattern to handle all field types
                quote! { #name::#variant { .. } => #cat_expr }
            })
            .collect();

        // Generate category enum
        let category_enum_name = format_ident!("{}Category", name);
        let category_variants: Vec<_> = sorted_categories
            .iter()
            .map(|c| format_ident!("{}", to_pascal_case(c)))
            .collect();
        let category_variant_names: Vec<_> = sorted_categories.clone();

        // Generate category_enum() method arms
        let category_enum_arms: Vec<_> = variant_categories
            .iter()
            .map(|(variant, cat)| {
                let cat_variant = match cat {
                    Some(c) => format_ident!("{}", to_pascal_case(c)),
                    None => format_ident!("Uncategorized"),
                };
                quote! { #name::#variant { .. } => #category_enum_name::#cat_variant }
            })
            .collect();

        // Generate is_* predicates
        let predicates: Vec<_> = sorted_categories
            .iter()
            .map(|cat| {
                let predicate_name = format_ident!("is_{}", cat);
                let cat_variants = categories.get(cat).unwrap();
                let patterns: Vec<_> = cat_variants
                    .iter()
                    .map(|v| quote! { #name::#v { .. } })
                    .collect();
                let doc = format!(
                    "Returns true if this action belongs to the `{}` category.",
                    cat
                );

                quote! {
                    #[doc = #doc]
                    pub fn #predicate_name(&self) -> bool {
                        matches!(self, #(#patterns)|*)
                    }
                }
            })
            .collect();

        // Add category-related implementations
        let category_enum_doc = format!(
            "Action categories for [`{}`].\n\n\
             Use [`{}::category_enum()`] to get the category of an action.",
            name, name
        );

        expanded = quote! {
            #expanded

            #[doc = #category_enum_doc]
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
            pub enum #category_enum_name {
                #(#category_variants,)*
                /// Actions that don't belong to any specific category.
                Uncategorized,
            }

            impl #category_enum_name {
                /// Get all category values
                pub fn all() -> &'static [Self] {
                    &[#(Self::#category_variants,)* Self::Uncategorized]
                }

                /// Get category name as string
                pub fn name(&self) -> &'static str {
                    match self {
                        #(Self::#category_variants => #category_variant_names,)*
                        Self::Uncategorized => "uncategorized",
                    }
                }
            }

            impl #name {
                /// Get the action's category (if categorized)
                pub fn category(&self) -> ::core::option::Option<&'static str> {
                    match self {
                        #(#category_arms_dedup,)*
                    }
                }

                /// Get the category as an enum value
                pub fn category_enum(&self) -> #category_enum_name {
                    match self {
                        #(#category_enum_arms,)*
                    }
                }

                #(#predicates)*
            }

            impl tui_dispatch::ActionCategory for #name {
                type Category = #category_enum_name;

                fn category(&self) -> ::core::option::Option<&'static str> {
                    #name::category(self)
                }

                fn category_enum(&self) -> Self::Category {
                    #name::category_enum(self)
                }
            }
        };

        // Generate dispatcher trait if requested
        if opts.generate_dispatcher {
            let dispatcher_trait_name = format_ident!("{}Dispatcher", name);

            let dispatch_methods: Vec<_> = sorted_categories
                .iter()
                .map(|cat| {
                    let method_name = format_ident!("dispatch_{}", cat);
                    let doc = format!("Handle actions in the `{}` category.", cat);
                    quote! {
                        #[doc = #doc]
                        fn #method_name(&mut self, action: &#name) -> bool {
                            false
                        }
                    }
                })
                .collect();

            let dispatch_arms: Vec<_> = sorted_categories
                .iter()
                .map(|cat| {
                    let method_name = format_ident!("dispatch_{}", cat);
                    let cat_variant = format_ident!("{}", to_pascal_case(cat));
                    quote! {
                        #category_enum_name::#cat_variant => self.#method_name(action)
                    }
                })
                .collect();

            let dispatcher_doc = format!(
                "Dispatcher trait for [`{}`].\n\n\
                 Implement the `dispatch_*` methods for each category you want to handle.\n\
                 The [`dispatch()`](Self::dispatch) method automatically routes to the correct handler.",
                name
            );

            expanded = quote! {
                #expanded

                #[doc = #dispatcher_doc]
                pub trait #dispatcher_trait_name {
                    #(#dispatch_methods)*

                    /// Handle uncategorized actions.
                    fn dispatch_uncategorized(&mut self, action: &#name) -> bool {
                        false
                    }

                    /// Main dispatch entry point - routes to category-specific handlers.
                    fn dispatch(&mut self, action: &#name) -> bool {
                        match action.category_enum() {
                            #(#dispatch_arms,)*
                            #category_enum_name::Uncategorized => self.dispatch_uncategorized(action),
                        }
                    }
                }
            };
        }
    }

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

            let name_arms = variant_names
                .iter()
                .zip(variant_strings.iter())
                .map(|(v, s)| {
                    quote! { #name::#v => #s }
                });

            let from_name_arms = variant_names
                .iter()
                .zip(variant_strings.iter())
                .map(|(v, s)| {
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
            let variant_strings: Vec<_> = variant_names.iter().map(|v| v.to_string()).collect();

            let name_arms = variant_names
                .iter()
                .zip(variant_strings.iter())
                .map(|(v, s)| {
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
