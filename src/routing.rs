use std::{collections::BTreeMap, future::Future};

use crate::error::{HttpError, HttpResult};
use duplicate::duplicate_item;
use futures::future::BoxFuture;
use hyper::{http::Method, Body, StatusCode};

type Handler<T> = Box<
    dyn Fn(T, BTreeMap<String, String>) -> BoxFuture<'static, HttpResult> + Send + Sync + 'static,
>;

#[derive(Clone, PartialEq, Eq)]
struct MethodOrd(hyper::Method);

impl PartialOrd for MethodOrd {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MethodOrd {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.as_str().cmp(other.0.as_str())
    }
}

pub struct Route<T> {
    methods: BTreeMap<MethodOrd, Handler<T>>,
}

impl<T> Route<T> {
    pub fn add<H, R>(mut self, method: Method, handler: H) -> Self
    where
        H: Fn(T, BTreeMap<String, String>) -> R + Send + Sync + 'static,
        R: Future<Output = HttpResult> + Send + 'static,
    {
        let handler: Handler<T> = Box::new(move |it, params| Box::pin(handler(it, params)));
        self.methods.insert(MethodOrd(method), handler);
        self
    }

    #[duplicate_item(
            fun      method;
            [get]    [GET];
            [post]   [POST];
            [put]    [PUT];
            [delete] [DELETE];
            [patch]  [PATCH]
          )]
    pub fn fun<H, R>(self, handler: H) -> Self
    where
        H: Fn(T, BTreeMap<String, String>) -> R + Send + Sync + 'static,
        R: Future<Output = HttpResult> + Send + 'static,
    {
        self.add(Method::method, handler)
    }

    fn new() -> Self {
        Self {
            methods: BTreeMap::new(),
        }
    }

    fn at(&self, method: Method) -> Option<&Handler<T>> {
        self.methods.get(&MethodOrd(method))
    }
}

#[duplicate_item(
        fun      method;
        [get]    [GET];
        [post]   [POST];
        [put]    [PUT];
        [delete] [DELETE];
        [patch]  [PATCH]
      )]
pub fn fun<T, H, R>(handler: H) -> Route<T>
where
    H: Fn(T, BTreeMap<String, String>) -> R + Send + Sync + 'static,
    R: Future<Output = HttpResult> + Send + 'static,
{
    Route::new().add(Method::method, handler)
}

pub struct Router<T> {
    inner: matchit::Router<Route<T>>,
}

impl<T> Router<T> {
    pub fn new<'a>(routes: Vec<(&'a str, Route<T>)>) -> Self {
        let mut inner = matchit::Router::new();
        for (route, value) in routes {
            inner.insert(route, value).unwrap();
        }
        Self { inner }
    }

    pub fn at<'a>(
        &'a self,
        path: &'a str,
        method: Method,
    ) -> Result<(BTreeMap<String, String>, &'a Handler<T>), HttpError> {
        let route = self
            .inner
            .at(path)
            .map_err(|_| HttpError::new(StatusCode::NOT_FOUND, Body::empty()))?;
        let map = route
            .params
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        route
            .value
            .at(method)
            .ok_or(HttpError::new(
                StatusCode::METHOD_NOT_ALLOWED,
                Body::empty(),
            ))
            .map(|h| (map, h))
    }
}
