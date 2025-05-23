//! Implementation of a [`FromStr`] derive macro.

#[cfg(doc)]
use std::str::FromStr;
use std::{collections::HashMap, iter};

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_quote, spanned::Spanned as _};

use crate::utils::Either;

/// Expands a [`FromStr`] derive macro.
pub fn expand(input: &syn::DeriveInput, _: &'static str) -> syn::Result<TokenStream> {
    match &input.data {
        syn::Data::Struct(data) => Ok(if data.fields.is_empty() {
            FlatExpansion::try_from(input)?.into_token_stream()
        } else {
            ForwardExpansion::try_from(input)?.into_token_stream()
        }),
        syn::Data::Enum(_) => Ok(FlatExpansion::try_from(input)?.into_token_stream()),
        syn::Data::Union(data) => Err(syn::Error::new(
            data.union_token.span(),
            "`FromStr` cannot be derived for unions",
        )),
    }
}

/// Expansion of a macro for generating a forwarding [`FromStr`] implementation of a struct.
struct ForwardExpansion<'i> {
    /// [`syn::Ident`] and [`syn::Generics`] of the struct.
    ///
    /// [`syn::Ident`]: struct@syn::Ident
    self_ty: (&'i syn::Ident, &'i syn::Generics),

    /// [`syn::Field`] representing the wrapped type to forward implementation on.
    inner: &'i syn::Field,
}

impl<'i> TryFrom<&'i syn::DeriveInput> for ForwardExpansion<'i> {
    type Error = syn::Error;

    fn try_from(input: &'i syn::DeriveInput) -> syn::Result<Self> {
        let syn::Data::Struct(data) = &input.data else {
            return Err(syn::Error::new(
                input.span(),
                "expected a struct for forward `FromStr` derive",
            ));
        };

        // TODO: Unite these two conditions via `&&` once MSRV is bumped to 1.88 or above.
        if data.fields.len() != 1 {
            return Err(syn::Error::new(
                data.fields.span(),
                "only structs with single field can derive `FromStr`",
            ));
        }
        let Some(inner) = data.fields.iter().next() else {
            return Err(syn::Error::new(
                data.fields.span(),
                "only structs with single field can derive `FromStr`",
            ));
        };

        Ok(Self {
            self_ty: (&input.ident, &input.generics),
            inner,
        })
    }
}

impl ToTokens for ForwardExpansion<'_> {
    /// Expands a forwarding [`FromStr`] implementations for a struct.
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let inner_ty = &self.inner.ty;
        let ty = self.self_ty.0;

        let mut generics = self.self_ty.1.clone();
        if !generics.params.is_empty() {
            generics.make_where_clause().predicates.push(parse_quote! {
                #inner_ty: derive_more::core::str::FromStr
            });
        }
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        let constructor = self.inner.self_constructor([parse_quote! { v }]);

        quote! {
            #[automatically_derived]
            impl #impl_generics derive_more::core::str::FromStr for #ty #ty_generics #where_clause {
                type Err = <#inner_ty as derive_more::core::str::FromStr>::Err;

                #[inline]
                fn from_str(s: &str) -> derive_more::core::result::Result<Self, Self::Err> {
                    derive_more::core::str::FromStr::from_str(s).map(|v| #constructor)
                }
            }
        }.to_tokens(tokens);
    }
}

/// Expansion of a macro for generating a flat [`FromStr`] implementation of an enum or a struct.
struct FlatExpansion<'i> {
    /// [`syn::Ident`] and [`syn::Generics`] of the enum/struct.
    ///
    /// [`syn::Ident`]: struct@syn::Ident
    self_ty: (&'i syn::Ident, &'i syn::Generics),

    /// [`syn::Ident`]s along with the matched values (enum variants or struct itself).
    ///
    /// [`syn::Ident`]: struct@syn::Ident
    matches: Vec<(
        &'i syn::Ident,
        Either<&'i syn::DataStruct, &'i syn::Variant>,
    )>,
}

impl<'i> TryFrom<&'i syn::DeriveInput> for FlatExpansion<'i> {
    type Error = syn::Error;

    fn try_from(input: &'i syn::DeriveInput) -> syn::Result<Self> {
        let matches = match &input.data {
            syn::Data::Struct(data) => {
                if !data.fields.is_empty() {
                    return Err(syn::Error::new(
                        data.fields.span(),
                        "only structs with no fields can derive `FromStr`",
                    ));
                }
                vec![(&input.ident, Either::Left(data))]
            }
            syn::Data::Enum(data) => data
                .variants
                .iter()
                .map(|variant| {
                    if !variant.fields.is_empty() {
                        return Err(syn::Error::new(
                            variant.fields.span(),
                            "only enums with no fields can derive `FromStr`",
                        ));
                    }
                    Ok((&variant.ident, Either::Right(variant)))
                })
                .collect::<syn::Result<_>>()?,
            syn::Data::Union(_) => {
                return Err(syn::Error::new(
                    input.span(),
                    "expected an enum or a struct for flat `FromStr` derive",
                ))
            }
        };

        Ok(Self {
            self_ty: (&input.ident, &input.generics),
            matches,
        })
    }
}

impl ToTokens for FlatExpansion<'_> {
    /// Expands a flat [`FromStr`] implementations for an enum.
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ty = self.self_ty.0;
        let (impl_generics, ty_generics, where_clause) =
            self.self_ty.1.split_for_impl();
        let ty_name = ty.to_string();

        let similar_lowercased = self
            .matches
            .iter()
            .map(|(v, _)| v.to_string().to_lowercase())
            .fold(<HashMap<_, u8>>::new(), |mut counts, v| {
                *counts.entry(v).or_default() += 1;
                counts
            });

        let match_arms = self.matches.iter().map(|(ident, value)| {
            let name = ident.to_string();
            let lowercased = name.to_lowercase();

            let exact_guard =
                (similar_lowercased[&lowercased] > 1).then(|| quote! { if s == #name });
            let constructor = value.self_constructor_empty();

            quote! { #lowercased #exact_guard => #constructor, }
        });

        quote! {
            #[allow(unreachable_code)] // for empty enums
            #[automatically_derived]
            impl #impl_generics derive_more::core::str::FromStr for #ty #ty_generics #where_clause {
                type Err = derive_more::FromStrError;

                fn from_str(
                    s: &str,
                ) -> derive_more::core::result::Result<Self, derive_more::FromStrError> {
                    derive_more::core::result::Result::Ok(match s.to_lowercase().as_str() {
                        #( #match_arms )*
                        _ => return derive_more::core::result::Result::Err(
                            derive_more::FromStrError::new(#ty_name),
                        ),
                    })
                }
            }
        }.to_tokens(tokens);
    }
}

/// Extension of [`syn::Fields`] used by this expansion.
trait FieldsExt {
    /// Generates a `name`d constructor with the provided `values` assigned to these
    /// [`syn::Fields`].
    ///
    /// # Panics
    ///
    /// If number of provided `values` doesn't match number of these [`syn::Fields`].
    fn constructor(
        &self,
        name: &syn::Path,
        values: impl IntoIterator<Item = syn::Ident>,
    ) -> TokenStream;

    /// Generates a `Self` type constructor with the provided `values` assigned to these
    /// [`syn::Fields`].
    ///
    /// # Panics
    ///
    /// If number of provided `values` doesn't match number of these [`syn::Fields`].
    fn self_constructor(
        &self,
        values: impl IntoIterator<Item = syn::Ident>,
    ) -> TokenStream {
        self.constructor(&self.self_ty(), values)
    }

    /// Generates a `Self` type constructor with no fields.
    ///
    /// # Panics
    ///
    /// If these [`syn::Fields`] are not [empty].
    ///
    /// [empty]: syn::Fields::is_empty
    fn self_constructor_empty(&self) -> TokenStream {
        self.self_constructor(iter::empty())
    }

    /// Returns a [`syn::Path`] representing a `Self` type of these [`syn::Fields`].
    fn self_ty(&self) -> syn::Path {
        parse_quote! { Self }
    }
}

impl FieldsExt for syn::Fields {
    fn constructor(
        &self,
        name: &syn::Path,
        values: impl IntoIterator<Item = syn::Ident>,
    ) -> TokenStream {
        let values = values.into_iter();
        let fields = match self {
            Self::Named(fields) => {
                let initializers = fields.named.iter().zip(values).map(|(f, value)| {
                    let ident = &f.ident;
                    quote! { #ident: #value }
                });
                Some(quote! { { #( #initializers, )*} })
            }
            Self::Unnamed(_) => Some(quote! { ( #( #values, )* ) }),
            Self::Unit => None,
        };
        quote! { #name #fields }
    }
}

impl FieldsExt for syn::Field {
    fn constructor(
        &self,
        name: &syn::Path,
        values: impl IntoIterator<Item = syn::Ident>,
    ) -> TokenStream {
        let mut values = values.into_iter();
        let value = values.next().expect("expected a single value");
        if values.next().is_some() {
            panic!("expected a single value");
        }

        if let Some(ident) = &self.ident {
            quote! { #name { #ident: #value } }
        } else {
            quote! { #name(#value) }
        }
    }
}

impl FieldsExt for syn::Variant {
    fn constructor(
        &self,
        name: &syn::Path,
        values: impl IntoIterator<Item = syn::Ident>,
    ) -> TokenStream {
        self.fields.constructor(name, values)
    }

    fn self_ty(&self) -> syn::Path {
        let variant = &self.ident;

        parse_quote! { Self::#variant }
    }
}

impl FieldsExt for syn::DataStruct {
    fn constructor(
        &self,
        name: &syn::Path,
        values: impl IntoIterator<Item = syn::Ident>,
    ) -> TokenStream {
        self.fields.constructor(name, values)
    }
}

impl<L: FieldsExt, R: FieldsExt> FieldsExt for Either<&L, &R> {
    fn constructor(
        &self,
        name: &syn::Path,
        values: impl IntoIterator<Item = syn::Ident>,
    ) -> TokenStream {
        match self {
            Self::Left(l) => l.constructor(name, values),
            Self::Right(r) => r.constructor(name, values),
        }
    }

    fn self_ty(&self) -> syn::Path {
        match self {
            Self::Left(l) => l.self_ty(),
            Self::Right(r) => r.self_ty(),
        }
    }
}
