//! Defines the HTTP-based UI. These endpoints generally return HTML and SVG.

use std::{sync::Arc, time::Duration};

use futures::{future, Future};
use hyper::{header, service::Service, Body, Method, Request, Response, StatusCode};
use ritz::{html, HtmlContent};

use crate::{
    imfs::ImfsFetcher,
    serve_session::ServeSession,
    web::{
        assets,
        interface::{ErrorResponse, SERVER_VERSION},
        util::json,
    },
};

pub struct UiService<F> {
    serve_session: Arc<ServeSession<F>>,
}

impl<F: ImfsFetcher> Service for UiService<F> {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = hyper::Error;
    type Future = Box<dyn Future<Item = Response<Self::ReqBody>, Error = Self::Error> + Send>;

    fn call(&mut self, request: Request<Self::ReqBody>) -> Self::Future {
        let response = match (request.method(), request.uri().path()) {
            (&Method::GET, "/") => self.handle_home(),
            (&Method::GET, "/logo.png") => self.handle_logo(),
            (&Method::GET, "/icon.png") => self.handle_icon(),
            (&Method::GET, "/show-instances") => self.handle_show_instances(),
            (&Method::GET, "/show-imfs") => self.handle_show_imfs(),
            (_method, path) => {
                return json(
                    ErrorResponse::not_found(format!("Route not found: {}", path)),
                    StatusCode::NOT_FOUND,
                )
            }
        };

        Box::new(future::ok(response))
    }
}

impl<F: ImfsFetcher> UiService<F> {
    pub fn new(serve_session: Arc<ServeSession<F>>) -> Self {
        UiService { serve_session }
    }

    fn handle_logo(&self) -> Response<Body> {
        Response::builder()
            .header(header::CONTENT_TYPE, "image/png")
            .body(Body::from(assets::logo()))
            .unwrap()
    }

    fn handle_icon(&self) -> Response<Body> {
        Response::builder()
            .header(header::CONTENT_TYPE, "image/png")
            .body(Body::from(assets::icon()))
            .unwrap()
    }

    fn handle_home(&self) -> Response<Body> {
        let page = self.normal_page(html! {
            <div class="button-list">
                { Self::button("Rojo Documentation", "https://rojo.space/docs") }
                { Self::button("View in-memory filesystem state", "/show-imfs") }
                { Self::button("View instance tree state", "/show-instances") }
            </div>
        });

        Response::builder()
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from(format!("<!DOCTYPE html>{}", page)))
            .unwrap()
    }

    fn handle_show_instances(&self) -> Response<Body> {
        Response::builder()
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from("TODO: /show-instances"))
            .unwrap()
    }

    fn handle_show_imfs(&self) -> Response<Body> {
        Response::builder()
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from("TODO: /show-imfs"))
            .unwrap()
    }

    fn stat_item<S: Into<String>>(name: &str, value: S) -> HtmlContent<'_> {
        html! {
            <span class="stat">
                <span class="stat-name">{ name } ": "</span>
                <span class="stat-value">{ value.into() }</span>
            </span>
        }
    }

    fn button<'a>(text: &'a str, href: &'a str) -> HtmlContent<'a> {
        html! {
            <a class="button" href={ href }>{ text }</a>
        }
    }

    fn normal_page<'a>(&'a self, body: HtmlContent<'a>) -> HtmlContent<'a> {
        let project_name = self.serve_session.project_name().unwrap_or("<unnamed>");
        let uptime = {
            let elapsed = self.serve_session.start_time().elapsed();

            // Round off all of our sub-second precision to make timestamps
            // nicer.
            let just_nanos = Duration::from_nanos(elapsed.subsec_nanos() as u64);
            let elapsed = elapsed - just_nanos;

            humantime::format_duration(elapsed).to_string()
        };

        Self::page(html! {
            <div class="root">
                <header class="header">
                    <img class="main-logo" src="/logo.png" />
                    <div class="stats">
                        { Self::stat_item("Server Version", SERVER_VERSION) }
                        { Self::stat_item("Project", project_name) }
                        { Self::stat_item("Server Uptime", uptime) }
                    </div>
                </header>
                <main class="main">
                    { body }
                </main>
            </div>
        })
    }

    fn page(body: HtmlContent<'_>) -> HtmlContent<'_> {
        html! {
            <html>
                <head>
                    <title>"Rojo Live Server"</title>
                    <link rel="icon" type="image/png" sizes="32x32" href="/icon.png" />
                    <meta name="viewport" content="width=device-width, initial-scale=1, minimum-scale=1, maximum-scale=1" />
                    <style>
                        { ritz::UnescapedText::new(assets::css()) }
                    </style>
                </head>

                <body>
                    { body }
                </body>
            </html>
        }
    }
}