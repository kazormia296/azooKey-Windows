use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::{
    parse_macro_input, Error, Expr, ExprTry, GenericArgument, Ident, ItemFn, PathArguments,
    ReturnType, Token, Type,
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
                "Expected 'ignore_with' or 'fail_with' parameter (e.g., #[anyhow(ignore_with = BOOL(0))] or #[anyhow(fail_with = E_FAIL)])"
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

// `?` 演算子を検出して書き換えるためのビジター
struct TryRewriter<'a> {
    fallback_arm: &'a proc_macro2::TokenStream,
}

impl<'a> VisitMut for TryRewriter<'a> {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        // 先に子要素（ネストされた `?` など）を再帰的に書き換える
        if let Expr::Try(ExprTry {
            expr: inner_expr,
            question_token,
            ..
        }) = expr
        {
            self.visit_expr_mut(inner_expr);

            let question_span = question_token.span();
            let fallback_arm = self.fallback_arm;

            // `?` トークンの Span を付与したコードへ書き換える
            // これにより、ここから出力される file!() や line!() などのロギング位置が
            // 元コードの `?` が記述されていた位置と一致するようになります。
            *expr = syn::parse2(quote_spanned! { question_span =>
                match #inner_expr {
                    Ok(v) => v,
                    Err(err) => {
                        let e = ::anyhow::Error::from(err);
                        ::tracing::error!("Error occurred: {:#?}", e);
                        if let Some(win_err) = e.downcast_ref::<::windows::core::Error>() {
                            return Err(win_err.clone());
                        } else {
                            return #fallback_arm;
                        }
                    }
                }
            })
            .unwrap();
        } else {
            syn::visit_mut::visit_expr_mut(self, expr);
        }
    }

    // クロージャ、非同期ブロック、内部定義関数の中身は
    // シグネチャの戻り値型と一致しなくなるため、書き換え処理をスキップ（除外）する
    fn visit_expr_closure_mut(&mut self, _i: &mut syn::ExprClosure) {}
    fn visit_expr_async_mut(&mut self, _i: &mut syn::ExprAsync) {}
    fn visit_item_mut(&mut self, _i: &mut syn::Item) {}
}

#[proc_macro_attribute]
pub fn anyhow(attr: TokenStream, input: TokenStream) -> TokenStream {
    // parse the input macro arguments
    let args = parse_macro_input!(attr as MacroArgs);

    // parse the input function
    let input_fn = parse_macro_input!(input as ItemFn);

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
    if !is_unit_type(output) {
        if let MacroArgs::Default = args {
            return Error::new_spanned(
                output,
                "For return types other than Result<()>, you must explicitly specify 'ignore_with = ...' or 'fail_with = ...' as a macro argument (e.g., #[anyhow(ignore_with = BOOL(0))])."
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

    // 元のシグネチャをベースに関数ボディだけを書き換える
    let mut modified_fn = input_fn.clone();

    // 関数の戻り値型を強制的に `::windows::core::Result<#output>` に置き換え
    modified_fn.sig.output = syn::parse2(quote! { -> ::windows::core::Result<#output> }).unwrap();

    // 関数本体のAST（ブロック）を再帰的に走査し、`?` をインラインマッチングに書き換える
    let mut rewriter = TryRewriter {
        fallback_arm: &fallback_arm,
    };
    rewriter.visit_block_mut(modified_fn.block.as_mut());

    let generated = quote! {
        #modified_fn
    };

    generated.into()
}
