use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Expr, Fields, LitStr, Meta, parse_macro_input};

// ── Parsed attributes ───────────────────────────────────────────────────────

struct ContainerAttrs {
    css: Option<Expr>,
    js: Option<Expr>,
    header: Option<Expr>,
    footer: Option<Expr>,
}

#[derive(Default)]
struct FieldAttrs {
    #[allow(dead_code)]
    is_secret: bool,
    #[allow(dead_code)]
    default: Option<Expr>,
    input_type: Option<String>,
}

// ── Entry point ─────────────────────────────────────────────────────────────

#[proc_macro_derive(Provision, attributes(provision))]
pub fn derive_provision(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return syn::Error::new_spanned(
                    s.fields.clone(),
                    "#[derive(Provision)] requires a struct with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "#[derive(Provision)] only supports structs")
                .to_compile_error()
                .into();
        }
    };

    let container_attrs = parse_container_attrs(&input.attrs);
    let from_form_body = generate_from_form(name, fields);
    let html_body = generate_html(name, fields, &container_attrs);
    let (to_bytes_body, from_bytes_body) = generate_storage(name, fields);

    let expanded = quote! {
        impl ::provisioner::config::ProvisionConfig for #name {
            const HTML: &'static str = {
                #html_body
            };

            fn from_form(body: &[u8]) -> Result<Self, ::provisioner::error::ParseError> {
                use ::provisioner::error::ParseError;
                use ::provisioner::form::{FormPairs, decode_into};

                let body_str = ::core::str::from_utf8(body)
                    .map_err(|_| ParseError::InvalidEncoding)?;

                #from_form_body
            }

            fn to_bytes(
                &self,
                buf: &mut [u8],
            ) -> Result<usize, ::provisioner::error::ParseError> {
                use ::provisioner::error::ParseError;
                #to_bytes_body
            }

            fn from_bytes(
                buf: &[u8],
            ) -> Result<Self, ::provisioner::error::ParseError> {
                use ::provisioner::error::ParseError;
                use ::provisioner::form::{FormPairs, decode_into};

                let body_str = ::core::str::from_utf8(buf)
                    .map_err(|_| ParseError::InvalidEncoding)?;

                #from_bytes_body
            }
        }
    };

    TokenStream::from(expanded)
}

// ── from_form codegen ───────────────────────────────────────────────────────

fn generate_from_form(
    name: &syn::Ident,
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
) -> proc_macro2::TokenStream {
    let field_idents: Vec<_> = fields.iter().map(|f| f.ident.as_ref().unwrap()).collect();
    let field_name_strs: Vec<String> = field_idents.iter().map(|n| n.to_string()).collect();
    let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();
    let buf_size = 256usize;

    let field_vars: Vec<_> = field_idents
        .iter()
        .map(|n| format_ident!("__field_{}", n))
        .collect();

    let declarations: Vec<_> = field_vars
        .iter()
        .zip(field_types.iter())
        .map(|(var, ty)| {
            quote! { let mut #var: Option<#ty> = None; }
        })
        .collect();

    let match_arms: Vec<_> = field_idents
        .iter()
        .zip(field_name_strs.iter())
        .zip(field_vars.iter())
        .zip(field_types.iter())
        .map(|(((_name, name_str), var), ty)| {
            quote! {
                #name_str => {
                    if #var.is_some() { continue; }
                    let decoded_val = decode_into(raw_value, &mut __buf)?;
                    let parsed = <#ty as ::core::str::FromStr>::from_str(decoded_val)
                        .map_err(|_| ParseError::InvalidValue(#name_str))?;
                    #var = Some(parsed);
                }
            }
        })
        .collect();

    let missing_checks: Vec<_> = field_idents
        .iter()
        .zip(field_vars.iter())
        .zip(field_name_strs.iter())
        .map(|((name, var), name_str)| {
            quote! { let #name = #var.ok_or(ParseError::MissingField(#name_str))?; }
        })
        .collect();

    let construct = {
        let field_inits: Vec<_> = field_idents.iter().map(|n| quote! { #n }).collect();
        quote! { #name { #(#field_inits),* } }
    };

    quote! {
        let mut __buf = [0u8; #buf_size];
        #(#declarations)*

        for (key, raw_value) in FormPairs::new(body_str) {
            match key {
                #(#match_arms)*
                _ => {}
            }
        }

        #(#missing_checks)*
        Ok(#construct)
    }
}

// ── HTML codegen ────────────────────────────────────────────────────────────

/// Generate the HTML constant as a single `&'static str` literal.
///
/// The full page (including default CSS, user customisations, and form
/// inputs) is assembled inside the proc macro at compile time on the host.
/// The emitted token stream is a plain string literal — no runtime work.
fn generate_html(
    _name: &syn::Ident,
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
    attrs: &ContainerAttrs,
) -> proc_macro2::TokenStream {
    let user_css = extract_string_literal(&attrs.css).unwrap_or_default();
    let user_js = extract_string_literal(&attrs.js).unwrap_or_default();
    let user_header = extract_string_literal(&attrs.header).unwrap_or_default();
    let user_footer = extract_string_literal(&attrs.footer).unwrap_or_default();

    let default_css = "*{box-sizing:border-box;margin:0;padding:0}body{font-family:system-ui,sans-serif;background:#1a1a2e;color:#e0e0e0;display:flex;justify-content:center;align-items:center;min-height:100vh}form{background:#16213e;padding:2rem;border-radius:8px;width:100%;max-width:400px;box-shadow:0 4px 24px rgba(0,0,0,.4)}h1{text-align:center;margin-bottom:1.5rem;color:#e94560}label{display:block;margin-bottom:1rem;font-weight:500}input{width:100%;padding:.6rem .8rem;margin-top:.3rem;border:1px solid #0f3460;border-radius:4px;background:#1a1a2e;color:#e0e0e0;font-size:1rem}input:focus{outline:2px solid #e94560;border-color:transparent}input[type=checkbox]{width:auto;margin-right:.5rem}button{width:100%;padding:.7rem;background:#e94560;color:#fff;border:none;border-radius:4px;font-size:1rem;cursor:pointer;margin-top:1rem}button:hover{background:#c23152}";

    let mut input_parts = Vec::new();
    for field in fields {
        let field_attrs = parse_field_attrs(&field.attrs);
        let name = field.ident.as_ref().unwrap().to_string();
        let label = to_label(&name);

        let input_type = field_attrs.input_type.as_deref().unwrap_or_else(|| {
            if field_attrs.is_secret {
                "password"
            } else if is_bool_type(&field.ty) {
                "checkbox"
            } else {
                "text"
            }
        });

        let default_val = field_attrs
            .default
            .as_ref()
            .map(|def| format!(" value=\"{}\"", escape_html(&expr_to_string(def))))
            .unwrap_or_default();

        if input_type == "checkbox" {
            input_parts.push(format!(
                "<label><input type=\"checkbox\" name=\"{name}\"{default_val}> {label}</label>",
            ));
        } else {
            input_parts.push(format!(
                "<label>{label}<input type=\"{input_type}\" name=\"{name}\"{default_val}></label>",
            ));
        }
    }
    let inputs = input_parts.join("");

    let mut html = String::new();
    html.push_str("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>WiFi Setup</title><style>");
    html.push_str(default_css);
    html.push_str(&user_css);
    html.push_str("</style></head><body>");
    html.push_str(&user_header);
    html.push_str("<form method=\"post\"><h1>WiFi Setup</h1>");
    html.push_str(&inputs);
    html.push_str("<button type=\"submit\">Save</button></form>");
    html.push_str(&user_footer);
    if !user_js.is_empty() {
        html.push_str("<script>");
        html.push_str(&user_js);
        html.push_str("</script>");
    }
    html.push_str("</body></html>");

    let lit = proc_macro2::Literal::string(&html);
    quote! { #lit }
}

/// Try to extract a string literal from a `syn::Expr`.
///
/// Works for both raw string literals (`"..."`) and `include_str!(...)`
/// (which is expanded to a literal before the proc macro runs).
fn extract_string_literal(expr: &Option<Expr>) -> Option<String> {
    match expr {
        Some(Expr::Lit(lit)) => match &lit.lit {
            syn::Lit::Str(s) => Some(s.value()),
            _ => None,
        },
        _ => None,
    }
}

// ── Storage (to_bytes / from_bytes) codegen ─────────────────────────────────

fn generate_storage(
    name: &syn::Ident,
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let field_idents: Vec<_> = fields.iter().map(|f| f.ident.as_ref().unwrap()).collect();
    let field_name_strs: Vec<String> = field_idents.iter().map(|n| n.to_string()).collect();

    // to_bytes
    let write_fields: Vec<_> = field_idents.iter().zip(field_name_strs.iter()).map(|(name, name_str)| {
        quote! {
            {
                let name_bytes = #name_str.as_bytes();
                if pos + name_bytes.len() + 1 > buf.len() {
                    return Err(ParseError::InvalidValue(#name_str));
                }
                buf[pos..pos + name_bytes.len()].copy_from_slice(name_bytes);
                pos += name_bytes.len();
                buf[pos] = b'=';
                pos += 1;

                let mut scratch = [0u8; 128];
                let val_bytes = {
                    let val_str = {
                        use ::core::fmt::Write;
                        let mut sw = ::provisioner::util::StringWriter { buf: &mut scratch, pos: 0 };
                        write!(sw, "{}", self.#name).map_err(|_| ParseError::InvalidValue(#name_str))?;
                        let len = sw.pos;
                        unsafe { ::core::str::from_utf8_unchecked(&scratch[..len]) }
                    };
                    val_str.as_bytes()
                };

                for &b in val_bytes {
                    match b {
                        b'&' => {
                            if pos + 3 > buf.len() { return Err(ParseError::InvalidValue(#name_str)); }
                            buf[pos] = b'%'; buf[pos+1] = b'2'; buf[pos+2] = b'6';
                            pos += 3;
                        }
                        b'=' => {
                            if pos + 3 > buf.len() { return Err(ParseError::InvalidValue(#name_str)); }
                            buf[pos] = b'%'; buf[pos+1] = b'3'; buf[pos+2] = b'D';
                            pos += 3;
                        }
                        _ => {
                            if pos + 1 > buf.len() { return Err(ParseError::InvalidValue(#name_str)); }
                            buf[pos] = b;
                            pos += 1;
                        }
                    }
                }

                if pos + 1 > buf.len() { return Err(ParseError::InvalidValue(#name_str)); }
                buf[pos] = b'&';
                pos += 1;
            }
        }
    }).collect();

    let to_bytes = quote! {
        let mut pos = 0usize;
        #(#write_fields)*
        Ok(pos)
    };

    // from_bytes
    let field_vars: Vec<_> = field_idents
        .iter()
        .map(|n| format_ident!("__field_{}", n))
        .collect();
    let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();

    let declarations: Vec<_> = field_vars
        .iter()
        .zip(field_types.iter())
        .map(|(var, ty)| {
            quote! { let mut #var: Option<#ty> = None; }
        })
        .collect();

    let match_arms: Vec<_> = field_idents
        .iter()
        .zip(field_name_strs.iter())
        .zip(field_vars.iter())
        .zip(field_types.iter())
        .map(|(((_name, name_str), var), ty)| {
            quote! {
                #name_str => {
                    if #var.is_some() { continue; }
                    let decoded_val = decode_into(raw_value, &mut __buf)?;
                    let parsed = <#ty as ::core::str::FromStr>::from_str(decoded_val)
                        .map_err(|_| ParseError::InvalidValue(#name_str))?;
                    #var = Some(parsed);
                }
            }
        })
        .collect();

    let missing_checks: Vec<_> = field_idents
        .iter()
        .zip(field_vars.iter())
        .zip(field_name_strs.iter())
        .map(|((name, var), name_str)| {
            quote! { let #name = #var.ok_or(ParseError::MissingField(#name_str))?; }
        })
        .collect();

    let construct = {
        let field_inits: Vec<_> = field_idents.iter().map(|n| quote! { #n }).collect();
        quote! { #name { #(#field_inits),* } }
    };

    let from_bytes = quote! {
        let mut __buf = [0u8; 256];
        #(#declarations)*

        for (key, raw_value) in FormPairs::new(body_str) {
            match key {
                #(#match_arms)*
                _ => {}
            }
        }

        #(#missing_checks)*
        Ok(#construct)
    };

    (to_bytes, from_bytes)
}

// ── Attribute parsing ───────────────────────────────────────────────────────

fn parse_container_attrs(attrs: &[syn::Attribute]) -> ContainerAttrs {
    let mut result = ContainerAttrs {
        css: None,
        js: None,
        header: None,
        footer: None,
    };

    for attr in attrs {
        if !attr.path().is_ident("provision") {
            continue;
        }
        if let Meta::List(list) = &attr.meta {
            let _ = list
                .parse_nested_meta(|meta| {
                    if meta.path.is_ident("css") {
                        result.css = Some(meta.value()?.parse()?);
                    } else if meta.path.is_ident("js") {
                        result.js = Some(meta.value()?.parse()?);
                    } else if meta.path.is_ident("header") {
                        result.header = Some(meta.value()?.parse()?);
                    } else if meta.path.is_ident("footer") {
                        result.footer = Some(meta.value()?.parse()?);
                    }
                    Ok(())
                })
                .ok();
        }
    }
    result
}

fn parse_field_attrs(attrs: &[syn::Attribute]) -> FieldAttrs {
    let mut result = FieldAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("provision") {
            continue;
        }
        if let Meta::List(list) = &attr.meta {
            let _ = list
                .parse_nested_meta(|meta| {
                    if meta.path.is_ident("secret") {
                        result.is_secret = true;
                    } else if meta.path.is_ident("default") {
                        result.default = Some(meta.value()?.parse()?);
                    } else if meta.path.is_ident("input_type") {
                        let v: LitStr = meta.value()?.parse()?;
                        result.input_type = Some(v.value());
                    }
                    Ok(())
                })
                .ok();
        }
    }
    result
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn to_label(name: &str) -> String {
    let mut result = String::new();
    let mut cap = true;
    for ch in name.chars() {
        if ch == '_' {
            result.push(' ');
            cap = true;
        } else if cap {
            result.push(ch.to_ascii_uppercase());
            cap = false;
        } else {
            result.push(ch);
        }
    }
    result
}

fn is_bool_type(ty: &syn::Type) -> bool {
    quote! { #ty }.to_string() == "bool"
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn expr_to_string(expr: &Expr) -> String {
    match expr {
        Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Str(s) => s.value(),
            _ => format!("{}", quote! { #expr }),
        },
        _ => format!("{}", quote! { #expr }),
    }
}
