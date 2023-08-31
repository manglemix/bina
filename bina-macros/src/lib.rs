use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{quote, format_ident};
use syn::{parse_macro_input, Token, Attribute, Visibility, Type, Expr, parse::{Parse, ParseStream}, DeriveInput};


struct ItemStatic {
    pub _attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub _static_token: Token![static],
    pub mutability: Option<Token![mut]>,
    pub ident: Ident,
    pub _colon_token: Token![:],
    pub ty: Type,
    pub _eq_token: Token![=],
    pub _expr: Expr,
    pub _semi_token: Token![;],
}

impl Parse for ItemStatic {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(ItemStatic {
            _attrs: input.call(Attribute::parse_outer)?,
            vis: input.parse()?,
            _static_token: input.parse()?,
            mutability: input.parse()?,
            ident: input.parse()?,
            _colon_token: input.parse()?,
            ty: input.parse()?,
            _eq_token: input.parse()?,
            _expr: input.parse()?,
            _semi_token: input.parse()?,
        })
    }
}


#[proc_macro]
pub fn static_reference(mut input: TokenStream) -> TokenStream {
    let cloned = input.clone();
    let ItemStatic { ident, ty, mutability, vis, .. } = parse_macro_input!(cloned);
    let ref_name = format_ident!("{ident}Reference");

    let result: TokenStream = if mutability.is_some() {
        quote! {
            #vis struct #ref_name;

            impl bina_ecs::reference::MutStaticReference for #ref_name {
                type Type = #ty;
                
                unsafe fn get() -> &'static #ty {
                    &#ident
                }
                unsafe fn get_mut () -> &'static mut #ty {
                    &mut #ident
                }
            }
        }.into()
    } else {
        quote! {
            #vis struct #ref_name;

            impl bina_ecs::reference::StaticReference for #ref_name {
                type Type = #ty;

                fn get() -> &'static #ty {
                    &#ident
                }
            }
        }.into()
    };

    input.extend(result);

    input
}

#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, .. } = parse_macro_input!(input);
    let static_name = format_ident!("_BINA_STORE_{ident}");
    let static_ref = format_ident!("_BINA_STORE_{ident}Reference");

    quote! {
        bina_macros::static_reference! {
            static mut #static_name: bina_ecs::component::ComponentStore<#ident> = bina_ecs::component::ComponentStore::new();
        }

        impl bina_ecs::component::Component for #ident {
            type StoreRef = #static_ref;


        }
    }.into()
}

#[proc_macro_derive(Singleton)]
pub fn derive_singleton(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, .. } = parse_macro_input!(input);

    quote! {
        impl bina_ecs::singleton::Singleton for #ident {
            fn get() -> &'static Self {
                use std::sync::OnceLock;
                static STORE: OnceLock<#ident> = OnceLock::new();
                STORE.get_or_init(Default::default)
            }
        }
    }.into()
}