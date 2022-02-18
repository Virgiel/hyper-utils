use hyper::{Body, Response, StatusCode};

pub type HttpResult = Result<Response<Body>, HttpError>;

pub struct HttpError {
    status: StatusCode,
    body: Body,
}

impl HttpError {
    pub fn new(status: StatusCode, body: Body) -> Self {
        Self { status, body }
    }

    pub fn response(self) -> Response<Body> {
        Response::builder()
            .status(self.status)
            .body(self.body)
            .unwrap()
    }
}

impl From<StatusCode> for HttpError {
    fn from(code: StatusCode) -> Self {
        Self::new(code, Body::empty())
    }
}

impl<E: std::error::Error> From<(StatusCode, E)> for HttpError {
    fn from((code, err): (StatusCode, E)) -> Self {
        eprintln!("{}", err);
        Self::new(code, Body::empty())
    }
}
pub trait ErrorHelper<O>: Sized {
    fn unexpected(self) -> O {
        self.status(StatusCode::INTERNAL_SERVER_ERROR)
    }
    fn bad_request(self) -> O {
        self.status(StatusCode::BAD_REQUEST)
    }
    fn status(self, status: StatusCode) -> O;
}

impl<T, E: std::error::Error> ErrorHelper<Result<T, HttpError>> for Result<T, E> {
    fn status(self, status: StatusCode) -> Result<T, HttpError> {
        self.map_err(|e| (status, e).into())
    }
}

impl<T> ErrorHelper<Result<T, HttpError>> for Option<T> {
    fn status(self, status: StatusCode) -> Result<T, HttpError> {
        self.ok_or_else(|| status.into())
    }
}

impl<E: std::error::Error> ErrorHelper<HttpError> for E {
    fn status(self, status: StatusCode) -> HttpError {
        (status, self).into()
    }
}
