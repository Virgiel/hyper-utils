use std::net::SocketAddr;

use futures::{future::BoxFuture, Future};
use hyper::{http::request::Parts, Body, Request, Response};

use crate::{
    client_ip,
    error::HttpResult,
    routing::{Route, Router},
};

pub struct Ctx<S> {
    pub addr: SocketAddr,
    pub state: S,
    pub parts: Parts,
}

impl<S> Ctx<S> {
    pub fn ip(&self) -> String {
        client_ip(&self.parts.headers)
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.addr.ip().to_string())
    }
}

pub struct App<S> {
    state: S,
    router: Router<(Ctx<S>, Body)>,
    pre: Box<
        dyn Fn(Request<Body>) -> BoxFuture<'static, Result<Request<Body>, Response<Body>>>
            + Send
            + Sync
            + 'static,
    >,
    post: Box<dyn Fn(Response<Body>) -> BoxFuture<'static, Response<Body>> + Send + Sync + 'static>,
}

impl<S: Clone + Send + Sync + 'static> App<S> {
    pub fn new(state: S) -> Self {
        Self {
            state,
            router: Router::new(vec![]),
            pre: Box::new(|req| Box::pin(async { Ok(req) })),
            post: Box::new(|resp| Box::pin(async { resp })),
        }
    }

    pub fn routes<'a>(mut self, routes: impl IntoIterator<Item =(&'a str, Route<(Ctx<S>, Body)>)>) -> Self {
        self.router = Router::new(routes);
        self
    }

    pub fn pre<H, R>(mut self, hook: H) -> Self
    where
        H: Fn(Request<Body>) -> R + Send + Sync + 'static,
        R: Future<Output = Result<Request<Body>, Response<Body>>> + Send + 'static,
    {
        self.pre = Box::new(move |req| Box::pin(hook(req)));
        self
    }

    pub fn post<H, R>(mut self, hook: H) -> Self
    where
        H: Fn(Response<Body>) -> R + Send + Sync + 'static,
        R: Future<Output = Response<Body>> + Send + 'static,
    {
        self.post = Box::new(move |req| Box::pin(hook(req)));
        self
    }

    async fn router_fn(
        state: S,
        addr: SocketAddr,
        req: Request<Body>,
        router: &Router<(Ctx<S>, Body)>,
    ) -> HttpResult {
        let (handler, params) = router.at(req.uri().path(), req.method())?;
        let params = params.iter().map(|(k, v)| (k.into(), v.into())).collect();
        let (parts, body) = req.into_parts();
        handler((Ctx { addr, state, parts }, body), params).await
    }

    pub async fn serve(&self, addr: SocketAddr, req: Request<Body>) -> Response<Body> {
        // Pre hook
        let req = (self.pre)(req).await;

        // Router
        let resp = match req {
            Ok(req) => Self::router_fn(self.state.clone(), addr, req, &self.router)
                .await
                .unwrap_or_else(|e| e.response()),
            Err(resp) => resp,
        };

        // Post hook
        (self.post)(resp).await
    }
}
