use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const FIRA_CODE_PATH: &str = "fonts/FiraCode-VF.woff2";
const MAIN_CSS_PATH: &str = "css/main.css";
const SITE_DIRECTORY: &str = "site";
const STATIC_DIRECTORY: &str = "site/static";
const STYLES_DIRECTORY: &str = "site/styles";
const FONT_URL_PLACEHOLDER: &str = "__FIRA_CODE_URL__";

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={SITE_DIRECTORY}");

    let content_files = files_below(Path::new(SITE_DIRECTORY))?
        .into_iter()
        .filter(|path| path.ends_with(".md"))
        .collect::<Vec<_>>();
    let static_files = files_below(Path::new(STATIC_DIRECTORY))?;
    let output_directory = PathBuf::from(std::env::var("OUT_DIR")?);
    let generated_css = generate_css(&static_files)?;
    fs::write(output_directory.join("main.css"), &generated_css)?;

    let mut generated = String::from("pub(super) static CONTENT_FILES: &[(&str, &str)] = &[\n");
    for path in content_files {
        writeln!(
            generated,
            "    ({path:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/site/\", {path:?}))),"
        )?;
    }

    generated.push_str(
        "];

pub(super) static STATIC_FILES: &[(&str, &str, &str, &[u8])] = &[
",
    );
    let stylesheet_path = fingerprinted_path(MAIN_CSS_PATH, content_hash(&generated_css))?;
    let stylesheet_url = format!("/static/{stylesheet_path}");
    writeln!(
        generated,
        "    ({MAIN_CSS_PATH:?}, {stylesheet_path:?}, {stylesheet_url:?}, include_bytes!(concat!(env!(\"OUT_DIR\"), \"/main.css\"))),"
    )?;
    for path in static_files {
        let bytes = fs::read(Path::new(STATIC_DIRECTORY).join(&path))?;
        let hash = content_hash(&bytes);
        let include_expression = format!(
            "include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/site/static/\", {path:?}))"
        );
        let fingerprinted_path = fingerprinted_path(&path, hash)?;
        let public_url = format!("/static/{fingerprinted_path}");
        writeln!(
            generated,
            "    ({path:?}, {fingerprinted_path:?}, {public_url:?}, {include_expression}),"
        )?;
    }
    generated.push_str(
        "];
",
    );

    fs::write(output_directory.join("embedded_files.rs"), generated)?;
    Ok(())
}

fn generate_css(static_files: &[String]) -> Result<Vec<u8>, Box<dyn Error>> {
    if !static_files.iter().any(|path| path == FIRA_CODE_PATH) {
        return Err(format!("missing static asset: {FIRA_CODE_PATH}").into());
    }

    let font = fs::read(Path::new(STATIC_DIRECTORY).join(FIRA_CODE_PATH))?;
    let font_path = fingerprinted_path(FIRA_CODE_PATH, content_hash(&font))?;
    let style_files = files_below(Path::new(STYLES_DIRECTORY))?
        .into_iter()
        .filter(|path| path.ends_with(".css"))
        .collect::<Vec<_>>();
    if style_files.is_empty() {
        return Err(format!("{STYLES_DIRECTORY} contains no CSS files").into());
    }

    let mut source = String::new();
    for path in style_files {
        let stylesheet = fs::read_to_string(Path::new(STYLES_DIRECTORY).join(&path))?;
        source.push_str(&stylesheet);
        if !stylesheet.ends_with('\n') {
            source.push('\n');
        }
        source.push('\n');
    }
    if source.matches(FONT_URL_PLACEHOLDER).count() != 1 {
        return Err(format!(
            "the merged stylesheet must contain {FONT_URL_PLACEHOLDER} exactly once"
        )
        .into());
    }

    Ok(source
        .replace(FONT_URL_PLACEHOLDER, &format!("/static/{font_path}"))
        .into_bytes())
}

fn content_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn fingerprinted_path(path: &str, hash: u64) -> Result<String, Box<dyn Error>> {
    let path = Path::new(path);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or("static asset must have a UTF-8 file name")?;
    let extension = path.extension().and_then(|extension| extension.to_str());
    let file_name = match extension {
        Some(extension) => format!("{stem}-{hash:016x}.{extension}"),
        None => format!("{stem}-{hash:016x}"),
    };
    Ok(
        match path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            Some(parent) => format!("{}/{file_name}", parent.display()),
            None => file_name,
        },
    )
}

fn files_below(root: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    files.sort_unstable();
    Ok(files)
}

fn visit(root: &Path, directory: &Path, files: &mut Vec<String>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with('.'))
        {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            visit(root, &entry.path(), files)?;
        } else if file_type.is_file() {
            let relative = entry.path().strip_prefix(root)?.to_path_buf();
            let path = relative
                .components()
                .map(|component| component.as_os_str().to_str())
                .collect::<Option<Vec<_>>>()
                .ok_or("embedded paths must be valid UTF-8")?
                .join("/");
            files.push(path);
        }
    }
    Ok(())
}
