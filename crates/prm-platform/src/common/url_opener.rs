//! Opening Azure DevOps and docs links in the default browser (allow-listed hosts, https only).

use prm_domain::{
    app::is_openable_url,
    ports::{PortError, PortErrorKind, UrlOpener},
};

/// Blocking (spawns the browser); the service calls it on the blocking pool.
#[derive(Copy, Clone, Debug, Default)]
pub struct BrowserOpener;

impl UrlOpener for BrowserOpener {
    fn open(&self, url: &url::Url) -> Result<(), PortError> {
        if !is_openable_url(url) {
            return Err(PortError::new(PortErrorKind::Denied, "URL not allowed"));
        }
        let mut options = webbrowser::BrowserOptions::new();
        options.with_suppress_output(true);
        webbrowser::open_browser_with_options(webbrowser::Browser::Default, url.as_str(), &options)
            .map_err(|e| PortError::new(PortErrorKind::Failed, e.to_string()))
    }
}

#[cfg(test)]
mod tests;
