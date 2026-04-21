pub mod diff;
#[cfg(feature = "cli")]
pub mod display;
pub mod function_info;
#[cfg(feature = "cli")]
pub mod mcp;
pub mod parser;
#[cfg(feature = "cli")]
pub mod print;
pub mod retaining_path;
pub mod snapshot;
#[cfg(feature = "cli")]
pub mod tui;
pub mod types;
pub mod utils;

/// Fetch the name of a Chrome extension from the Chrome Web Store.
#[cfg(feature = "cli")]
pub fn resolve_chrome_extension_name(extension_id: &str) -> Option<String> {
    use std::time::Duration;
    let url = format!("https://chromewebstore.google.com/detail/{extension_id}");
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .build()
        .new_agent();
    let body: String = agent
        .get(&url)
        .call()
        .ok()?
        .body_mut()
        .read_to_string()
        .ok()?;
    let title_start = body.find("<title>")? + "<title>".len();
    let title_end = body[title_start..].find("</title>")? + title_start;
    let title = &body[title_start..title_end];
    let name = title.strip_suffix(" - Chrome Web Store")?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}
