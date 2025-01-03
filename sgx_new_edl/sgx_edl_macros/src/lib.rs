use std::str::FromStr;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, punctuated::Punctuated, token::Comma, visit::Visit, visit_mut::VisitMut,
    ForeignItemFn, GenericArgument, Ident, ItemFn, Lifetime, PathArguments, Token, Type, TypePath,
    Visibility,
};

struct ReplaceLifetimes;

impl VisitMut for ReplaceLifetimes {
    fn visit_lifetime_mut(&mut self, i: &mut Lifetime) {
        *i = syn::parse_quote!('a);
    }
}

struct GenericExtractor {
    tys: Vec<Type>,
    //lifetimes: Vec<Lifetime>,
}

impl Visit<'_> for GenericExtractor {
    fn visit_type_path(&mut self, i: &TypePath) {
        for segment in &i.path.segments {
            if let PathArguments::AngleBracketed(ref args) = segment.arguments {
                for arg in &args.args {
                    match arg {
                        //GenericArgument::Lifetime(lifetime) => {
                        //    self.lifetimes.push(lifetime.clone())
                        //}
                        GenericArgument::Type(ty) => self.tys.push(ty.clone()),
                        _ => {}
                    }
                }
            }
        }
    }

    fn visit_type_reference(&mut self, i: &syn::TypeReference) {
        syn::visit::visit_type_reference(self, i);
    }

    fn visit_type(&mut self, i: &Type) {
        match i {
            Type::Path(type_path) => self.visit_type_path(type_path),
            _ => syn::visit::visit_type(self, i),
        }
    }
}

#[proc_macro]
pub fn ecalls(input: TokenStream) -> TokenStream {
    let s = input.to_string().split(';').collect::<Vec<_>>().join(";,");
    let token = TokenStream::from_str(&s).unwrap();
    let parser = Punctuated::<ForeignItemFn, Token![,]>::parse_terminated;
    let fns = parse_macro_input!(token with parser);
    let extern_fns = gen_extern_func(&fns);
    let tab = gen_ecall_table(&fns);
    let fn_mods = gen_fn_mods(&fns);
    quote! {
        #[cfg(feature = "enclave")]
        #extern_fns
        #[cfg(feature = "enclave")]
        #tab
        #fn_mods
    }
    .into()
}

fn gen_fn_mods(fns: &Punctuated<ForeignItemFn, Comma>) -> proc_macro2::TokenStream {
    let mods = fns.iter().enumerate().map(|(idx, f)| {
        let sig = &f.sig;
        let args = sig.inputs.iter().collect::<Vec<_>>();
        let args_name = args.iter().map(|arg| match arg {
            syn::FnArg::Receiver(_) => unimplemented!(),
            syn::FnArg::Typed(pat_type) => pat_type.pat.as_ref(),
        });
        let fn_name = &sig.ident;
        quote! {
            #[cfg(feature = "app")]
            pub mod #fn_name {
                use super::*;

                pub const IDX: usize = #idx;

                pub fn ecall(eid: usize, otab: &[sgx_new_edl::OTabEntry], #(#args),*) -> sgx_types::error::SgxStatus {
                    sgx_new_edl::untrust_ecall(IDX, eid, otab, (#(#args_name),*))
                }
            }
        }
    });
    quote! {
        #(#mods)*
    }
}

fn gen_extern_func(fns: &Punctuated<ForeignItemFn, Comma>) -> proc_macro2::TokenStream {
    let fn_names = fns.iter().map(|f| &f.sig.ident).map(|id| externed_name(id));
    let ret = fns.iter().map(|f| &f.sig.output);
    quote! {
        extern "C" {
            #(
                fn #fn_names(args: *const u8) #ret;
            )*
        }
    }
}

fn gen_ecall_table(fns: &Punctuated<ForeignItemFn, Comma>) -> proc_macro2::TokenStream {
    let ids = fns.iter().map(|f| &f.sig.ident).map(|id| {
        let extern_name = externed_name(id);
        quote! {
            #extern_name
        }
    });
    quote! {
        pub static ECALL_TABLE: &[unsafe extern "C" fn(*const u8) -> sgx_types::error::SgxStatus] = &[
            #(#ids),*
        ];
    }
}

fn externed_name(ident: &Ident) -> Ident {
    Ident::new(&format!("_ecall_{}", ident), ident.span())
}

#[proc_macro_attribute]
pub fn ecall(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut f = parse_macro_input!(item as ItemFn);
    let mut raw_fn = f.clone();
    let sig = &mut f.sig;

    sig.inputs.iter_mut().for_each(|arg| {
        ReplaceLifetimes.visit_fn_arg_mut(arg);
    });

    let mut ex = GenericExtractor {
        tys: Vec::new(),
        //lifetimes: Vec::new(),
    };

    let fn_name = &sig.ident;
    let extern_name = externed_name(fn_name);
    let (arg_names, arg_tys): (Vec<_>, Vec<_>) = sig
        .inputs
        .iter()
        .map(|arg| match arg {
            syn::FnArg::Receiver(_) => unimplemented!(),
            syn::FnArg::Typed(pat_type) => {
                ex.visit_type(&pat_type.ty);
                (pat_type.pat.as_ref(), pat_type.ty.as_ref())
            }
        })
        .unzip();

    raw_fn.vis = Visibility::Inherited;

    let tys = ex.tys;

    quote! {
        pub mod #fn_name {
            use super::*;

            struct _PhantomMarker<'a> {
                _phantom: &'a ()
            }

            impl<'a> Default for _PhantomMarker<'a> {
                fn default() -> Self {
                    Self {
                        _phantom: &()
                    }
                }
            }

            impl<'a> sgx_new_edl::Ecall<(#(#tys), *)> for _PhantomMarker<'a> {
                type Args = (#(#arg_tys), *);

                fn call(&self, args: Self::Args) -> sgx_types::error::SgxStatus {
                    let (#(#arg_names), *) = args;
                    #fn_name(#(#arg_names), *)
                }
            }

            #[no_mangle]
            pub extern fn #extern_name(args: *const u8) -> sgx_types::error::SgxStatus {
                sgx_new_edl::EcallWrapper::wrapper_t(&_PhantomMarker::default(), args)
            }

            #raw_fn
        }
    }
    .into()
}