#![feature(ptr_from_ref)]
use std::{ops::Deref, path::Path};

use image::io::Reader as ImageReader;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse::Parse, parse_macro_input, Data, DeriveInput, Fields, Ident, Lit, LitByteStr, Token,
    Type, Visibility,
};

// #[proc_macro_derive(Component, attributes(improve))]
#[proc_macro]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let DeriveInput {
        vis,
        ident,
        data,
        attrs,
        generics,
    } = parse_macro_input!(input);

    let Data::Struct(data) = data else {
        return quote! { compile_error!("This macro can only handle structs") }.into();
    };
    let Fields::Named(data) = data.fields else {
        return quote! { compile_error!("This macro can only handle named fields") }.into();
    };
    let fields = data.named;
    let ref_ident = format_ident!("{ident}Reference");
    let mut process_modifier_fields = Vec::new();
    let mut new_struct_data = Vec::new();

    let ref_data: Vec<_> = fields
        .iter()
        .map(|field| {
            if let Some(attr) = field.attrs.last() {
                if attr.meta.path().to_token_stream().to_string() == "improve" {
                    let Type::Path(path) = &field.ty else {
                        return quote! { compile_error!("Unexpected type") }.into();
                    };
                    let ident = field.ident.as_ref().unwrap();
                    let ty = &field.ty;

                    match path.to_token_stream().to_string().as_str() {
                        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32"
                        | "i64" | "i128" | "isize" | "f32" | "f64" => {
                            process_modifier_fields.push(ident);
                            new_struct_data
                                .push(quote! { #ident: bina::ecs::component::NumberField<#ty>, });
                            quote! { #ident: bina::ecs::component::NumberFieldRef<'a, #ty>, }
                        }
                        _ => {
                            new_struct_data.push(quote! { #ident: #ty, });
                            quote! { #ident: &'a #ty, }
                        }
                    }
                } else {
                    return quote! { compile_error!("Unexpected attribute") }.into();
                }
            } else {
                let ident = &field.ident;
                let ty = &field.ty;
                quote! { #ident: &'a #ty, }
            }
        })
        .collect();

    let get_ref_body = fields.iter().map(|field| {
        let ident = field.ident.as_ref().unwrap();
        if process_modifier_fields.contains(&ident) {
            quote! {
                #ident: self.#ident.get_ref(),
            }
        } else {
            quote! {
                #ident: &self.#ident,
            }
        }
    });
    let flush_body = process_modifier_fields.iter().map(|ident| {
        quote! { bina::ecs::component::ComponentField::process_modifiers(&mut self.#ident); }
    });

    quote! {
        #(#attrs)*
        #vis struct #ident #generics {
            #(#new_struct_data)*
        }

        #vis struct #ref_ident<'a> {
            #(#ref_data)*
            _phantom: std::marker::PhantomData<&'a ()>
        }

        impl bina::ecs::component::Component for #ident {
            type Reference<'a> = #ref_ident<'a>;

            fn get_ref<'a>(&'a self) -> Self::Reference<'a> {
                #ref_ident {
                    #(#get_ref_body)*
                    _phantom: std::marker::PhantomData
                }
            }
            fn flush(&mut self) {
                #(#flush_body)*
            }
        }

    }
    .into()
}

struct ImageInput {
    pub vis: Visibility,
    pub ident: Ident,
    pub _eq_token: Token![=],
    pub path: Lit,
}

impl Parse for ImageInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            vis: input.parse()?,
            ident: input.parse()?,
            _eq_token: input.parse()?,
            path: input.parse()?,
        })
    }
}

#[proc_macro]
pub fn load_image(input: TokenStream) -> TokenStream {
    let ImageInput {
        vis, ident, path, ..
    } = parse_macro_input!(input);
    let Lit::Str(path) = path else {
        return quote! { compile_error!("Path must be a string literal") }.into();
    };
    let path = path.to_token_stream().to_string();
    let path = path.trim_matches('"');

    let img = match ImageReader::open(path) {
        Ok(x) => x,
        Err(e) => {
            let msg = format!(
                "Failed to load image at {:?}: {e:?}",
                AsRef::<Path>::as_ref(".")
                    .canonicalize()
                    .unwrap()
                    .join(path)
            );
            return quote! { compile_error!(#msg) }.into();
        }
    };
    let Ok(img) = img.decode() else {
        return quote! { compile_error!("Image is invalid") }.into();
    };
    let img = img.to_rgba8();

    let bytes = LitByteStr::new(img.deref(), Span::call_site());
    let bytes = Lit::ByteStr(bytes);
    let width = img.width();
    let height = img.height();

    quote! {
        #vis static #ident: bina::graphics::raw_img::RawImg<bina::graphics::image::Rgba<u8>> = unsafe { bina::graphics::raw_img::RawImg::new(#width, #height, #bytes) };
    }.into()
}
