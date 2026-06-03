use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, Data, DeriveInput, Error, Expr, Fields, GenericArgument, PathArguments,
    Result, Type,
};

#[proc_macro_derive(Provision, attributes(provision))]
pub fn derive_provision(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match derive_provision_inner(input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

struct FieldInfo {
    name: syn::Ident,
    ty: Type,
    #[allow(dead_code)]
    is_heapless_string: bool,
    #[allow(dead_code)]
    attrs: FieldAttrs,
}

#[allow(dead_code)]
struct FieldAttrs {
    default: Option<Expr>,
    secret: bool,
    input_type: Option<String>,
}

#[allow(dead_code)]
struct ContainerAttrs {
    css: Option<Expr>,
    js: Option<Expr>,
    header: Option<Expr>,
    footer: Option<Expr>,
}

fn derive_provision_inner(input: DeriveInput) -> Result<TokenStream2> {
    let struct_name = &input.ident;

    let named_fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            Fields::Unnamed(_) => {
                return Err(Error::new_spanned(
                    &input.ident,
                    "#[derive(Provision)] only supports structs with named fields",
                ))
            }
            Fields::Unit => {
                return Err(Error::new_spanned(
                    &input.ident,
                    "#[derive(Provision)] does not support unit structs",
                ))
            }
        },
        Data::Enum(e) => {
            return Err(Error::new_spanned(
                e.enum_token,
                "#[derive(Provision)] cannot be applied to enums",
            ))
        }
        Data::Union(u) => {
            return Err(Error::new_spanned(
                u.union_token,
                "#[derive(Provision)] cannot be applied to unions",
            ))
        }
    };

    let _container_attrs = parse_container_attrs(&input.attrs)?;

    let fields: Vec<FieldInfo> = named_fields
        .iter()
        .map(|f| {
            let name = f.ident.clone().unwrap();
            let ty = f.ty.clone();
            let is_heapless_string = is_heapless_string_type(&ty);
            let attrs = parse_field_attrs(&f.attrs)?;
            Ok(FieldInfo { name, ty, is_heapless_string, attrs })
        })
        .collect::<Result<_>>()?;

    let from_form_body = generate_from_form(&fields);

    Ok(quote! {
        impl ::provisioner::ProvisionConfig for #struct_name {
            const HTML: &'static str = "";

            fn from_form(body: &[u8]) -> ::core::result::Result<Self, ::provisioner::error::ParseError> {
                #from_form_body
            }

            fn to_bytes(&self, _buf: &mut [u8]) -> ::core::result::Result<usize, ::provisioner::error::ParseError> {
                todo!()
            }

            fn from_bytes(_buf: &[u8]) -> ::core::result::Result<Self, ::provisioner::error::ParseError> {
                todo!()
            }
        }
    })
}

fn generate_from_form(fields: &[FieldInfo]) -> TokenStream2 {
    let field_vars: Vec<_> = fields
        .iter()
        .map(|f| {
            let var = format_ident!("field_{}", f.name);
            let ty = &f.ty;
            quote! {
                let mut #var: ::core::option::Option<#ty> = ::core::option::Option::None;
            }
        })
        .collect();

    let match_arms: Vec<_> = fields
        .iter()
        .map(|f| {
            let key = f.name.to_string();
            let var = format_ident!("field_{}", f.name);
            quote! {
                #key => {
                    #var = ::core::option::Option::Some(
                        val.parse().map_err(|_| ::provisioner::error::ParseError::InvalidValue(#key))?
                    );
                }
            }
        })
        .collect();

    let struct_fields: Vec<_> = fields
        .iter()
        .map(|f| {
            let name = &f.name;
            let key = f.name.to_string();
            let var = format_ident!("field_{}", f.name);
            quote! {
                #name: #var.ok_or(::provisioner::error::ParseError::MissingField(#key))?
            }
        })
        .collect();

    quote! {
        let body_str = ::core::str::from_utf8(body)
            .map_err(|_| ::provisioner::error::ParseError::InvalidEncoding)?;
        #(#field_vars)*
        for (key, val) in ::provisioner::form::decode(body_str) {
            match key {
                #(#match_arms)*
                _ => {}
            }
        }
        ::core::result::Result::Ok(Self {
            #(#struct_fields,)*
        })
    }
}

fn parse_container_attrs(attrs: &[syn::Attribute]) -> Result<ContainerAttrs> {
    let mut result = ContainerAttrs { css: None, js: None, header: None, footer: None };

    for attr in attrs {
        if !attr.path().is_ident("provision") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("css") {
                result.css = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("js") {
                result.js = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("header") {
                result.header = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("footer") {
                result.footer = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error("unknown provision container attribute"));
            }
            Ok(())
        })?;
    }

    Ok(result)
}

fn parse_field_attrs(attrs: &[syn::Attribute]) -> Result<FieldAttrs> {
    let mut result = FieldAttrs { default: None, secret: false, input_type: None };

    for attr in attrs {
        if !attr.path().is_ident("provision") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("secret") {
                result.secret = true;
            } else if meta.path.is_ident("default") {
                result.default = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("input_type") {
                let lit: syn::LitStr = meta.value()?.parse()?;
                result.input_type = Some(lit.value());
            } else {
                return Err(meta.error("unknown provision field attribute"));
            }
            Ok(())
        })?;
    }

    Ok(result)
}

/// Returns true if `ty` looks like `heapless::String<N>` or `String<N>`.
/// Used to identify string fields for HTML input-type inference (Feature 2).
fn is_heapless_string_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let segs = &type_path.path.segments;
    let last = match segs.len() {
        1 => &segs[0],
        2 if segs[0].ident == "heapless" => &segs[1],
        _ => return false,
    };
    if last.ident != "String" {
        return false;
    }
    matches!(
        &last.arguments,
        PathArguments::AngleBracketed(args)
            if args.args.len() == 1
                && matches!(args.args.first(), Some(GenericArgument::Const(_)))
    )
}
