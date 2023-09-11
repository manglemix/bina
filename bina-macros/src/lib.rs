use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse::Parse, parse_macro_input, DeriveInput, Ident, ItemStatic, StaticMutability, Token, Type,
    Visibility,
};

// struct CreateUniverseInput {
//     vis: Visibility,
//     universe: Ident,

// }

// #[proc_macro]
// pub fn create_universe(input: TokenStream) -> TokenStream {

// }

// #[proc_macro]
// pub fn static_reference(mut input: TokenStream) -> TokenStream {
//     let cloned = input.clone();
//     let ItemStatic {
//         ident,
//         ty,
//         mutability,
//         vis,
//         ..
//     } = parse_macro_input!(cloned);
//     let ref_name = format_ident!("{ident}Reference");

//     let result: TokenStream = if matches!(mutability, StaticMutability::Mut(_)) {
//         quote! {
//             #vis struct #ref_name;

//             impl bina::ecs::reference::MutStaticReference for #ref_name {
//                 type Type = #ty;

//                 unsafe fn get() -> &'static #ty {
//                     &#ident
//                 }
//                 unsafe fn get_mut () -> &'static mut #ty {
//                     &mut #ident
//                 }
//             }
//         }
//         .into()
//     } else {
//         quote! {
//             #vis struct #ref_name;

//             impl bina::ecs::reference::StaticReference for #ref_name {
//                 type Type = #ty;

//                 fn get() -> &'static #ty {
//                     &#ident
//                 }
//             }
//         }
//         .into()
//     };

//     input.extend(result);

//     input
// }

// #[proc_macro_attribute]
// pub fn component(attr: TokenStream, input: TokenStream) -> TokenStream {
//     if !attr.is_empty() {
//         return quote! { compile_error!("component macro takes no attributes") }.into();
//     }

//     let cloned = input.clone();
//     let mut item_trait: syn::ItemImpl = parse_macro_input!(cloned);

//     let ident = item_trait.self_ty.to_token_stream();
//     let static_name = format_ident!("_BINA_STORE_{ident}");
//     let static_ref = format_ident!("_BINA_STORE_{ident}Reference");

//     item_trait.items.push({
//         syn::ImplItem::Type(syn::ImplItemType {
//             attrs: vec![],
//             type_token: Default::default(),
//             ident: Ident::new("StoreRef", Span::call_site()),
//             generics: Default::default(),
//             ty: Type::Verbatim(static_ref.into_token_stream()),
//             semi_token: Default::default(),
//             vis: syn::Visibility::Inherited,
//             defaultness: Default::default(),
//             eq_token: Default::default(),
//         })
//     });

//     let mut result: TokenStream = static_reference(quote! {
//         static #static_name: std::cell::SyncUnsafeCell<bina::ecs::component::ComponentStore<#ident>> = std::cell::SyncUnsafeCell::new(bina::ecs::component::ComponentStore::new());
//     }.into());
//     result.extend::<TokenStream>(item_trait.into_token_stream().into());

//     result
// }

// #[proc_macro_derive(Singleton)]
// pub fn derive_singleton(input: TokenStream) -> TokenStream {
//     let DeriveInput { ident, .. } = parse_macro_input!(input);

//     quote! {
//         impl bina::ecs::singleton::Singleton for #ident {
//             fn get() -> &'static Self {
//                 use std::sync::OnceLock;
//                 static STORE: OnceLock<#ident> = OnceLock::new();
//                 STORE.get_or_init(Default::default)
//             }
//         }
//     }
//     .into()
// }

// struct RegisterComponentArgs {
//     universe: Ident,
//     _comma: Token![,],
//     component: syn::TypePath,
// }

// impl Parse for RegisterComponentArgs {
//     fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
//         Ok(Self {
//             universe: input.parse()?,
//             _comma: input.parse()?,
//             component: input.parse()?,
//         })
//     }
// }

// #[proc_macro]
// pub fn register_component(input: TokenStream) -> TokenStream {
//     let RegisterComponentArgs {
//         universe,
//         component,
//         ..
//     } = parse_macro_input!(input);

//     quote! {
//         impl bina::ecs::universe::RegisteredComponent for #component { }
//         #universe.register_component::<#component>();
//     }
//     .into()
// }
