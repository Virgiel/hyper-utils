use std::fmt::Write;
use std::str::FromStr;

use proc_macro::Delimiter;
use proc_macro::TokenStream;
use proc_macro::TokenTree;
use proc_macro_error::abort;
use proc_macro_error::proc_macro_error;

#[proc_macro]
#[proc_macro_error]
pub fn routes(item: TokenStream) -> TokenStream {
    let mut iter = item.into_iter();
    let mut routes = Vec::new();
    while let Some(tree) = iter.next() {
        let path = parse_path(tree);
        let route = parse_methods(iter.next());
        routes.push((path, route));
    }
    let mut out = String::new();
    out.push('[');
    for (path, methods) in routes {
        write!(out, "({},", path.path).unwrap();
        let mut iter = methods.into_iter();
        if let Some((method, function)) = iter.next() {
            out_method(&mut out, &path.args, &method, function);
        }
        for (method, function) in iter {
            out.push('.');
            out_method(&mut out, &path.args, &method, function);
        }
        writeln!(out, "),").unwrap()
    }
    out.push(']');
    TokenStream::from_str(&out).unwrap()
}

fn out_method(out: &mut impl Write, args: &[String], method: &str, function: String) {
    write!(out, "{method}(").unwrap();
    out_function(out, args, function);
    write!(out, ")").unwrap();
}

fn out_function(out: &mut impl Write, args: &[String], function: String) {
    write!(out, "|(ctx, body), p| async move {{").unwrap();
    for arg in args {
        write!(out, "let {arg} = p.get(\"{arg}\").unwrap();").unwrap();
    }
    write!(out, "{function} }}").unwrap();
}

#[derive(Debug)]

struct Path {
    path: String,
    args: Vec<String>,
}

fn parse_path(tree: TokenTree) -> Path {
    if let TokenTree::Literal(lit) = &tree {
        let path = lit.to_string();
        if path.starts_with("\"/") {
            return Path {
                args: path
                    .trim_matches('"')
                    .split('/')
                    .filter_map(|s| s.starts_with([':', '*']).then(|| s[1..].to_string()))
                    .collect(),
                path,
            };
        }
    }
    abort!(tree.span(), "Expected a route string like '\"/route\"'")
}

fn parse_method(tree: TokenTree) -> String {
    if let TokenTree::Ident(i) = &tree {
        return i.to_string().to_lowercase();
    }
    abort!(tree.span(), "Expected method")
}

fn parse_sep(tree: Option<TokenTree>) {
    if let Some(tree) = tree {
        if let TokenTree::Punct(p) = &tree {
            if p.as_char() == ':' {
                return;
            }
        }
        abort!(tree.span(), "Expected ':'")
    }
    panic!("Missing ':'")
}

fn parse_function(tree: Option<TokenTree>) -> String {
    if let Some(tree) = tree {
        match &tree {
            TokenTree::Group(g) => {
                if let Delimiter::Brace = g.delimiter() {
                    g.to_string()
                } else {
                    abort!(g.span(), "Expected function");
                }
            }
            _ => abort!(tree.span(), "Expected function"),
        }
    } else {
        panic!("Missing function")
    }
}

fn parse_methods(tree: Option<TokenTree>) -> Vec<(String, String)> {
    if let Some(tree) = tree {
        if let TokenTree::Group(group) = &tree {
            if let Delimiter::Brace = group.delimiter() {
                let mut stream = group.stream().into_iter();
                let mut methods = Vec::new();
                while let Some(tree) = stream.next() {
                    let method = parse_method(tree);
                    parse_sep(stream.next());
                    let function = parse_function(stream.next());
                    methods.push((method, function));
                }
                return methods;
            }
        }
        panic!(
            "Expected route methods like `{{}}` got '{}'",
            tree.to_string()
        )
    } else {
        Vec::new()
    }
}
