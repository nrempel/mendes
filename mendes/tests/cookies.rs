#![cfg(feature = "cookies")]

use std::convert::TryInto;
use std::sync::Arc;

use async_trait::async_trait;
use mendes::application::IntoResponse;
use mendes::cookies::{cookie, AppWithAeadKey, AppWithCookies, Key};
use mendes::http::header::{COOKIE, SET_COOKIE};
use mendes::http::request::Parts;
use mendes::http::{Request, Response, StatusCode};
use mendes::{handler, route, Application, Context};
use serde::{Deserialize, Serialize};

#[tokio::test]
async fn cookie() {
    let app = Arc::new(App {
        key: mendes::cookies::Key::new(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31,
        ]),
    });

    let rsp = App::handle(Context::new(app.clone(), path_request("/store"))).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    let set = rsp.headers().get(SET_COOKIE).unwrap();
    let value = set.to_str().unwrap().split(';').next().unwrap();

    let mut req = path_request("/extract");
    req.headers_mut().insert(COOKIE, value.try_into().unwrap());
    let rsp = App::handle(Context::new(app, req)).await;
    assert_eq!(rsp.status(), StatusCode::OK);
    assert_eq!(rsp.into_body(), "user = 37");
}

fn path_request(path: &str) -> Request<()> {
    Request::builder()
        .uri(format!("https://example.com{}", path))
        .body(())
        .unwrap()
}

struct App {
    key: mendes::cookies::Key,
}

impl AppWithAeadKey for App {
    fn key(&self) -> &Key {
        &self.key
    }
}

#[async_trait]
impl Application for App {
    type RequestBody = ();
    type ResponseBody = String;
    type Error = Error;

    async fn handle(mut cx: Context<Self>) -> Response<Self::ResponseBody> {
        route!(match cx.path() {
            Some("store") => store,
            Some("extract") => extract,
        })
    }
}

#[handler(GET)]
async fn extract(app: &App, req: &http::request::Parts) -> Result<Response<String>, Error> {
    let session = app.cookie::<Session>(&req.headers).unwrap();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(format!("user = {}", session.user))
        .unwrap())
}

#[handler(GET)]
async fn store(app: &App) -> Result<Response<String>, Error> {
    let session = Session { user: 37 };
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(SET_COOKIE, app.set_cookie_header(Some(session)).unwrap())
        .body("Hello, world".into())
        .unwrap())
}

#[cookie]
#[derive(Deserialize, Serialize)]
struct Session {
    user: i32,
}

#[derive(Debug)]
enum Error {
    Mendes(mendes::Error),
}

impl From<mendes::Error> for Error {
    fn from(e: mendes::Error) -> Self {
        Error::Mendes(e)
    }
}

impl From<&Error> for StatusCode {
    fn from(e: &Error) -> StatusCode {
        let Error::Mendes(e) = e;
        StatusCode::from(e)
    }
}

impl IntoResponse<App> for Error {
    fn into_response(self, _: &App, _: &Parts) -> Response<String> {
        let Error::Mendes(err) = self;
        Response::builder()
            .status(StatusCode::from(&err))
            .body(err.to_string())
            .unwrap()
    }
}
