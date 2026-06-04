use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{
    parse_macro_input, Error, Expr, GenericArgument, Ident, ItemFn, PathArguments, ReturnType,
    Token, Type,
};

enum MacroArgs {
    Default,
    OkFallback(Expr),
    ErrFallback(Expr),
}

impl Parse for MacroArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(MacroArgs::Default);
        }

        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let value: Expr = input.parse()?;

        if key == "ignore_with" {
            Ok(MacroArgs::OkFallback(value))
        } else if key == "fail_with" {
            Ok(MacroArgs::ErrFallback(value))
        } else {
            Err(Error::new(
                key.span(),
                "Expected 'default' or 'err' parameter (e.g., #[anyhow(ignore_with = BOOL(0))] or #[anyhow(fail_with = E_FAIL)])"
            ))
        }
    }
}

fn is_unit_type(ty: &Type) -> bool {
    match ty {
        Type::Tuple(type_tuple) => type_tuple.elems.is_empty(),
        _ => false,
    }
}

/// 戻り値の型から `Result<T>` の `T`（Ok側の型）を抽出するヘルパー
fn extract_result_type(return_type: &ReturnType) -> Result<&Type, TokenStream> {
    if let ReturnType::Type(_, ty) = return_type {
        if let Type::Path(type_path) = &**ty {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident == "Result" {
                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(GenericArgument::Type(ok_type)) = args.args.first() {
                            return Ok(ok_type);
                        }
                    }
                }
            }
        }
        Err(
            Error::new_spanned(ty, "Expected a Result<T, anyhow::Error> return type")
                .to_compile_error()
                .into(),
        )
    } else {
        Err(
            Error::new_spanned(return_type, "Expected a Result return type")
                .to_compile_error()
                .into(),
        )
    }
}

#[proc_macro_attribute]
/// `anyhow::Result` を `windows::core::Result` に変換する属性マクロ。
///
/// このマクロは、関数の戻り値が `anyhow::Result<T>` であることを前提としています。関数内で発生したエラーは `anyhow::Error` としてキャッチされ、以下のルールに従って `windows::core::Result<T>` に変換されます。
/// - `anyhow::Error` のチェーンに `windows::core::Error` が含まれている場合、そのエラーが優先的に返されます。
/// - それ以外のエラーは、マクロ引数に基づいて以下のように処理されます。
///   - `#[anyhow]`（引数なし）: `Ok(Default::default())` を返す（ただし、戻り値の型が `()` 以外の場合はコンパイルエラー）。
///   - `#[anyhow(fallback_to_default = EXPR)]`: `Ok(EXPR)` を返す（ただし、戻り値の型が `()` 以外の場合はコンパイルエラー）。
///   - `#[anyhow(fallback_err = EXPR)]`: `Err(windows::core::Error::from(EXPR))` を返す（ただし、戻り値の型が `()` 以外の場合はコンパイルエラー）。    
///
/// このマクロでは、`windows::core::Error`は返すようにしていますが、そうでないエラーは基本的にログに記録して無視する形になります。これは、TSF Application側が適切なエラーハンドリングを行っていない場合に予期せぬエラーを避けるためです。
///
/// 例:
/// ```rust
/// #[anyhow]
/// fn example() -> Result<()> {
///     // 何らかの処理
///     Ok(())
/// }
///
/// #[anyhow(fallback_to_default = BOOL(0))]
/// fn example_with_default() -> Result<BOOL> {
///     // 何らかの処理
///     Ok(BOOL(1))
/// }
///
/// #[anyhow(fallback_err = E_FAIL)]
/// fn example_with_err() -> Result<()> {
///     // 何らかの処理
///     Ok(())
/// }
/// ```
pub fn anyhow(attr: TokenStream, input: TokenStream) -> TokenStream {
    // parse the input macro arguments
    let args = parse_macro_input!(attr as MacroArgs);

    // parse the input function
    let input_fn = parse_macro_input!(input as ItemFn);

    // get the function name, inputs, and body
    let fn_name = &input_fn.sig.ident;
    let fn_inputs = &input_fn.sig.inputs;
    let fn_body = &input_fn.block;

    // check if the function has a return type
    let output = match &input_fn.sig.output {
        ReturnType::Type(_, _ty) => {
            let result = extract_result_type(&input_fn.sig.output);

            match result {
                Ok(ok_type) => ok_type,
                Err(err) => return err,
            }
        }
        _ => {
            return Error::new_spanned(&input_fn.sig, "Expected a Result return type")
                .to_compile_error()
                .into();
        }
    };

    // Safety Guardrail:
    // If the return type is not `()`, we must enforce developers to specify either `ok = ...` or `fallback_err = ...`
    // to prevent accidental silent failure with unhandled default values.
    if !is_unit_type(output) {
        if let MacroArgs::Default = args {
            return Error::new_spanned(
                output,
                "For return types other than Result<()>, you must explicitly specify 'ok = ...' or 'fallback_err = ...' as a macro argument (e.g., #[anyhow(ok = BOOL(0))])."
            )
            .to_compile_error()
            .into();
        }
    }

    // Determine the fallback behavior based on macro arguments
    let fallback_arm = match args {
        MacroArgs::Default => {
            quote! { Ok(Default::default()) }
        }
        MacroArgs::OkFallback(expr) => {
            quote! { Ok(#expr) }
        }
        MacroArgs::ErrFallback(expr) => {
            quote! { Err(::windows::core::Error::from(#expr)) }
        }
    };

    // generate the new function
    let generated = quote! {
        fn #fn_name(#fn_inputs) -> ::windows::core::Result<#output> {
            let result: ::anyhow::Result<#output> = (|| #fn_body)();

            match result {
                Ok(v) => Ok(v),
                Err(e) => {
                    ::tracing::error!("Error internally occurred: {:?}", e);

                    // Prioritize propagating the original windows::core::Error if it exists in the anyhow chain
                    if let Some(win_err) = e.downcast_ref::<::windows::core::Error>() {
                        Err(win_err.clone())
                    } else {
                        // Fallback to the configured behavior for other internal errors
                        #fallback_arm
                    }
                }
            }
        }
    };

    generated.into()
}
