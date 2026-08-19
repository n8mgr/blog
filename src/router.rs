use crate::embedded;
use crate::render::{HomePage, Post, Site};
use askama::Template;
use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    page: &'a HomePage,
    recent_posts: &'a [Post],
    stylesheet_url: &'static str,
}

#[derive(Template)]
#[template(path = "posts.html")]
struct PostsTemplate<'a> {
    posts: &'a [Post],
    stylesheet_url: &'static str,
    search_script_url: &'static str,
}

#[derive(Template)]
#[template(path = "post.html")]
struct PostTemplate<'a> {
    post: &'a Post,
    stylesheet_url: &'static str,
}

#[derive(Template)]
#[template(path = "not_found.html")]
struct NotFoundTemplate {
    stylesheet_url: &'static str,
}

pub fn create_router(site: Site) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/posts", get(posts))
        .route("/posts/{slug}", get(post))
        .route("/feed.xml", get(rss_feed))
        .route("/search-index.json", get(search_index))
        .route("/static/{*path}", get(asset))
        .fallback(not_found)
        .with_state(Arc::new(site))
}

async fn index(State(site): State<Arc<Site>>) -> Response {
    render_template(IndexTemplate {
        page: &site.home,
        recent_posts: &site.posts[..site.posts.len().min(3)],
        stylesheet_url: asset_url("css/main.css"),
    })
}

async fn posts(State(site): State<Arc<Site>>) -> Response {
    render_template(PostsTemplate {
        posts: &site.posts,
        stylesheet_url: asset_url("css/main.css"),
        search_script_url: asset_url("js/search.js"),
    })
}

async fn post(Path(slug): Path<String>, State(site): State<Arc<Site>>) -> Response {
    match site.posts.iter().find(|post| post.slug == slug) {
        Some(post) => render_template(PostTemplate {
            post,
            stylesheet_url: asset_url("css/main.css"),
        }),
        None => not_found().await,
    }
}

async fn search_index(State(site): State<Arc<Site>>) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        site.search_index.clone(),
    )
        .into_response()
}

async fn rss_feed(State(site): State<Arc<Site>>) -> Response {
    (
        [(header::CONTENT_TYPE, "application/rss+xml; charset=utf-8")],
        site.rss_feed.clone(),
    )
        .into_response()
}

async fn asset(Path(path): Path<String>) -> Response {
    let Some(file) = embedded::static_file(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = mime_guess::from_path(&path).first_or_octet_stream();
    (
        [
            (header::CONTENT_TYPE, content_type.as_ref()),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        Body::from(file),
    )
        .into_response()
}

async fn not_found() -> Response {
    let mut response = render_template(NotFoundTemplate {
        stylesheet_url: asset_url("css/main.css"),
    });
    *response.status_mut() = StatusCode::NOT_FOUND;
    response
}

fn asset_url(path: &str) -> &'static str {
    embedded::asset_url(path).unwrap_or_else(|| panic!("missing embedded asset: {path}"))
}

fn render_template(template: impl Template) -> Response {
    match template.render() {
        Ok(markup) => Html(markup).into_response(),
        Err(error) => {
            eprintln!("template rendering failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{HomePage, PostMeta};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use chrono::{TimeZone, Utc};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    const POST_PATH: &str = "/posts/test-post";

    fn test_site() -> Site {
        Site {
            home: HomePage {
                description: "Test site".to_owned(),
                body_html: "<h1>Test site</h1>".to_owned(),
            },
            posts: vec![Post {
                slug: "test-post".to_owned(),
                meta: PostMeta {
                    title: "Test Post".to_owned(),
                    description: Some("Test description".to_owned()),
                    tags: vec!["test".to_owned()],
                },
                date: Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap(),
                body_html: "<p>Test post</p>".to_owned(),
                date_label: "January 2, 2026".to_owned(),
            }],
            search_index: "[]".to_owned(),
            rss_feed: "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><link>https://n8m.us/posts/test-post</link></channel></rss>".to_owned(),
        }
    }

    #[tokio::test]
    async fn returns_the_not_found_page_with_its_status() {
        let response = create_router(test_site())
            .oneshot(
                Request::builder()
                    .uri("/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let markup = String::from_utf8(body.to_vec()).unwrap();
        assert!(markup.contains("<title>~/n8m.us/404</title>"));
    }

    #[tokio::test]
    async fn fingerprints_and_immutably_caches_public_assets() {
        let stylesheet_url = asset_url("css/main.css");
        assert!(stylesheet_url.starts_with("/static/css/main-"));
        assert!(stylesheet_url.ends_with(".css"));
        assert!(!stylesheet_url.contains('?'));

        let response = create_router(test_site())
            .oneshot(
                Request::builder()
                    .uri(stylesheet_url)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let stylesheet = String::from_utf8(body.to_vec()).unwrap();
        assert!(stylesheet.contains(asset_url("fonts/FiraCode-VF.woff2")));
    }

    #[tokio::test]
    async fn serves_the_search_index_as_json() {
        let response = create_router(test_site())
            .oneshot(
                Request::builder()
                    .uri("/search-index.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/json; charset=utf-8"
        );
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"[]");
    }

    #[tokio::test]
    async fn post_previews_are_full_area_links() {
        let app = create_router(test_site());
        for (path, link_class) in [("/", "post-card-link"), ("/posts", "post-row-link")] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let markup = String::from_utf8(body.to_vec()).unwrap();
            assert!(
                markup.contains(&format!("class=\"{link_class}\" href=\"{POST_PATH}\"")),
                "{path} should render its post preview as a full-area link"
            );
        }
    }

    #[tokio::test]
    async fn document_titles_read_like_paths() {
        let app = create_router(test_site());
        for (path, title) in [
            ("/", "~/n8m.us"),
            ("/posts", "~/n8m.us/posts"),
            (POST_PATH, "~/n8m.us/posts/Test Post"),
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let markup = String::from_utf8(body.to_vec()).unwrap();
            assert!(
                markup.contains(&format!("<title>{title}</title>")),
                "{path} should have the title {title}"
            );
        }
    }

    #[tokio::test]
    async fn serves_a_discoverable_rss_feed() {
        let app = create_router(test_site());
        let feed_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/feed.xml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            feed_response.headers()[header::CONTENT_TYPE],
            "application/rss+xml; charset=utf-8"
        );
        let feed = feed_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let feed = String::from_utf8(feed.to_vec()).unwrap();
        assert!(feed.contains("<rss version=\"2.0\""));
        assert!(feed.contains("<link>https://n8m.us/posts/test-post</link>"));

        let home_response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let home = home_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let home = String::from_utf8(home.to_vec()).unwrap();
        assert!(home.contains("rel=\"alternate\" type=\"application/rss+xml\""));
        assert!(home.contains("href=\"/feed.xml\""));
    }
}
