use crate::embedded;
use chrono::{DateTime, NaiveDate, Utc};
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd, html};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

const ORIGIN: &str = "https://n8m.us";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PostMeta {
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
}

pub(crate) struct Post {
    pub(crate) slug: String,
    pub(crate) meta: PostMeta,
    pub(crate) date: DateTime<Utc>,
    pub(crate) source: &'static str,
    pub(crate) body_html: String,
    pub(crate) date_label: String,
}

pub(crate) struct HomePage {
    pub(crate) description: String,
    pub(crate) source: &'static str,
    pub(crate) body_html: String,
}

#[derive(Serialize)]
struct SearchItem {
    slug: String,
    title: String,
    description: String,
    tags: Vec<String>,
}

#[derive(Debug)]
pub struct SiteError(String);

impl SiteError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for SiteError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SiteError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HomeMeta {
    #[serde(default)]
    description: String,
}

pub struct Site {
    pub(crate) home: HomePage,
    pub(crate) posts: Vec<Post>,
    pub(crate) search_index: String,
    pub(crate) rss_feed: String,
    pub(crate) sitemap: String,
}

pub fn load_site() -> Result<Site, SiteError> {
    let home_source = embedded_content("index.md")?;
    let (home_frontmatter, home_markdown) = split_frontmatter(home_source)?;
    let home_meta: HomeMeta = parse_frontmatter(home_frontmatter, "index.md")?;
    let home = HomePage {
        description: home_meta.description,
        source: home_source,
        body_html: markdown_to_html(home_markdown),
    };

    let mut posts = embedded::content_files()
        .map(|(path, _)| path)
        .filter(|path| is_post_path(path))
        .map(load_post)
        .collect::<Result<Vec<_>, _>>()?;

    posts.sort_by_key(|post| Reverse(post.date));

    let search_items = posts
        .iter()
        .map(|post| SearchItem {
            slug: post.slug.clone(),
            title: post.meta.title.clone(),
            description: post.meta.description.clone().unwrap_or_default(),
            tags: post.meta.tags.clone(),
        })
        .collect::<Vec<_>>();
    let search_index = serde_json::to_string(&search_items)
        .map_err(|error| SiteError::new(format!("could not build search index: {error}")))?;
    let rss_feed = build_rss_feed(&home, &posts);
    let sitemap = build_sitemap(&posts);

    Ok(Site {
        home,
        posts,
        search_index,
        rss_feed,
        sitemap,
    })
}

fn is_post_path(path: &str) -> bool {
    path.starts_with("posts/") && path.ends_with(".md")
}

fn build_rss_feed(home: &HomePage, posts: &[Post]) -> String {
    let feed_url = format!("{ORIGIN}/feed.xml");
    let mut feed = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n\
         <channel>\n\
         <title>~/n8m.us</title>\n\
         <link>{}/</link>\n\
         <description>{}</description>\n\
         <language>en-us</language>\n\
         <atom:link href=\"{}\" rel=\"self\" type=\"application/rss+xml\" />\n",
        ORIGIN,
        escape_xml(&home.description),
        escape_xml(&feed_url)
    );

    if let Some(post) = posts.first() {
        feed.push_str(&format!(
            "<lastBuildDate>{}</lastBuildDate>\n",
            post.date.to_rfc2822()
        ));
    }

    for post in posts {
        let post_url = format!("{ORIGIN}/posts/{}", post.slug);
        let post_url = escape_xml(&post_url);
        feed.push_str(&format!(
            "<item>\n\
             <title>{}</title>\n\
             <link>{post_url}</link>\n\
             <guid isPermaLink=\"true\">{post_url}</guid>\n\
             <pubDate>{}</pubDate>\n",
            escape_xml(&post.meta.title),
            post.date.to_rfc2822()
        ));
        if let Some(description) = &post.meta.description {
            feed.push_str(&format!(
                "<description>{}</description>\n",
                escape_xml(description)
            ));
        }
        for tag in &post.meta.tags {
            feed.push_str(&format!("<category>{}</category>\n", escape_xml(tag)));
        }
        feed.push_str("</item>\n");
    }

    feed.push_str("</channel>\n</rss>\n");
    feed
}

fn build_sitemap(posts: &[Post]) -> String {
    let mut sitemap = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    let latest = posts.first().map(|post| &post.date);
    push_sitemap_url(&mut sitemap, &format!("{ORIGIN}/"), latest);
    for post in posts {
        push_sitemap_url(
            &mut sitemap,
            &format!("{ORIGIN}/posts/{}", post.slug),
            Some(&post.date),
        );
    }
    sitemap.push_str("</urlset>\n");
    sitemap
}

fn push_sitemap_url(sitemap: &mut String, location: &str, last_modified: Option<&DateTime<Utc>>) {
    sitemap.push_str("<url>\n");
    sitemap.push_str(&format!("<loc>{}</loc>\n", escape_xml(location)));
    if let Some(last_modified) = last_modified {
        sitemap.push_str(&format!(
            "<lastmod>{}</lastmod>\n",
            last_modified.format("%Y-%m-%d")
        ));
    }
    sitemap.push_str("</url>\n");
}

fn escape_xml(source: &str) -> String {
    source
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn load_post(path: &str) -> Result<Post, SiteError> {
    let source = embedded_content(path)?;
    let (frontmatter, markdown) =
        split_frontmatter(source).map_err(|error| SiteError::new(format!("{path}: {error}")))?;
    let meta: PostMeta = parse_frontmatter(frontmatter, path)?;
    let slug = path
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".md"))
        .ok_or_else(|| SiteError::new(format!("invalid post path: {path}")))?
        .to_owned();
    let date = post_date(&slug).map_err(|error| SiteError::new(format!("{path}: {error}")))?;
    let date_label = date.format("%B %e, %Y").to_string();
    let body_html = markdown_to_html(markdown);

    Ok(Post {
        slug,
        meta,
        date,
        source,
        body_html,
        date_label,
    })
}

fn post_date(slug: &str) -> Result<DateTime<Utc>, SiteError> {
    let date = slug
        .get(..10)
        .filter(|_| slug.as_bytes().get(10) == Some(&b'-'))
        .ok_or_else(|| SiteError::new("post filename must start with YYYY-MM-DD-"))?;
    let date = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| SiteError::new("post filename must start with a valid YYYY-MM-DD- date"))?;
    Ok(date.and_hms_opt(0, 0, 0).unwrap().and_utc())
}

fn embedded_content(path: &str) -> Result<&'static str, SiteError> {
    embedded::content(path)
        .ok_or_else(|| SiteError::new(format!("missing embedded content: {path}")))
}

fn parse_frontmatter<T>(source: &str, path: &str) -> Result<T, SiteError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(source)
        .map_err(|error| SiteError::new(format!("invalid frontmatter in {path}: {error}")))
}

pub fn markdown_to_html(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_SMART_PUNCTUATION;
    let parser = Parser::new_ext(markdown, options);
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();
    let theme = &theme_set.themes["base16-ocean.dark"];
    let mut events = Vec::new();
    let mut parser = parser.peekable();

    while let Some(event) = parser.next() {
        let Event::Start(Tag::CodeBlock(kind)) = event else {
            events.push(event);
            continue;
        };

        let language = match &kind {
            CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or("text"),
            CodeBlockKind::Indented => "text",
        };
        let mut code = String::new();
        for inner in parser.by_ref() {
            match inner {
                Event::End(TagEnd::CodeBlock) => break,
                Event::Text(text) | Event::Code(text) => code.push_str(&text),
                Event::SoftBreak | Event::HardBreak => code.push('\n'),
                _ => {}
            }
        }

        let syntax = syntax_set
            .find_syntax_by_token(language)
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
        let rendered = highlighted_html_for_string(&code, &syntax_set, syntax, theme)
            .unwrap_or_else(|_| format!("<pre><code>{}</code></pre>", escape_html(&code)));
        events.push(Event::Html(CowStr::Boxed(rendered.into_boxed_str())));
    }

    let mut output = String::new();
    html::push_html(&mut output, events.into_iter());
    output
}

fn split_frontmatter(content: &str) -> Result<(&str, &str), SiteError> {
    let mut lines = content.split_inclusive('\n');
    let first = lines
        .next()
        .ok_or_else(|| SiteError::new("content is empty"))?;
    if first.trim_end_matches(['\r', '\n']) != "---" {
        return Err(SiteError::new(
            "content must start with a frontmatter fence",
        ));
    }

    let frontmatter_start = first.len();
    let mut offset = frontmatter_start;
    for line in lines {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Ok((
                &content[frontmatter_start..offset],
                content[offset + line.len()..].trim_start(),
            ));
        }
        offset += line.len();
    }

    Err(SiteError::new("frontmatter is missing its closing fence"))
}

fn escape_html(source: &str) -> String {
    source
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn identifies_post_paths() {
        assert!(is_post_path("posts/example.md"));
        assert!(!is_post_path("pages/example.md"));
        assert!(!is_post_path("posts/example.txt"));
    }

    #[test]
    fn derives_the_post_date_from_its_slug() {
        assert_eq!(
            post_date("2026-08-19-example").unwrap(),
            Utc.with_ymd_and_hms(2026, 8, 19, 0, 0, 0).unwrap()
        );
        assert!(post_date("example").is_err());
        assert!(post_date("2026-02-30-example").is_err());
    }

    #[test]
    fn renders_markdown_features_and_highlights_code() {
        let output = markdown_to_html("~~old~~\n\n```rust\nfn main() {}\n```");
        assert!(output.contains("<del>old</del>"));
        assert!(output.contains("background-color"));
    }

    #[test]
    fn reports_missing_frontmatter() {
        let error = split_frontmatter("# No frontmatter").unwrap_err();
        assert!(error.to_string().contains("frontmatter"));
    }

    #[test]
    fn builds_rss() {
        let home = HomePage {
            description: "Test & feed".to_owned(),
            source: "# Test feed",
            body_html: String::new(),
        };
        let posts = [Post {
            slug: "test-post".to_owned(),
            meta: PostMeta {
                title: "Test <Post>".to_owned(),
                description: Some("A \"description\"".to_owned()),
                tags: vec!["rust".to_owned(), "storage & systems".to_owned()],
            },
            date: Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap(),
            source: "# Test Post",
            body_html: String::new(),
            date_label: "January 2, 2026".to_owned(),
        }];
        let feed = build_rss_feed(&home, &posts);

        assert!(feed.contains("<atom:link href=\"https://n8m.us/feed.xml\" rel=\"self\""));
        assert!(feed.contains("<guid isPermaLink=\"true\">https://n8m.us/posts/test-post</guid>"));
        assert!(feed.contains("<description>Test &amp; feed</description>"));
        assert!(feed.contains("<title>Test &lt;Post&gt;</title>"));
        assert!(feed.contains("<description>A &quot;description&quot;</description>"));
        assert!(feed.contains("<category>rust</category>"));
        assert!(feed.contains("<category>storage &amp; systems</category>"));
    }

    #[test]
    fn builds_sitemap() {
        let posts = [Post {
            slug: "test-post".to_owned(),
            meta: PostMeta {
                title: "Test Post".to_owned(),
                description: None,
                tags: Vec::new(),
            },
            date: Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap(),
            source: "# Test Post",
            body_html: String::new(),
            date_label: "January 2, 2026".to_owned(),
        }];
        let sitemap = build_sitemap(&posts);

        assert!(sitemap.contains("<loc>https://n8m.us/</loc>"));
        assert!(sitemap.contains("<loc>https://n8m.us/posts/test-post</loc>"));
        assert!(sitemap.contains("<lastmod>2026-01-02</lastmod>"));
    }
}
