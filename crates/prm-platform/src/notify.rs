//! Toast checks that do not need a running notifier.

use prm_domain::ports::PortError;

/// Loads the XML the way Windows will (`XmlDocument::LoadXml`); `Ok` elsewhere.
pub fn validate_toast_xml(xml: &str) -> Result<(), PortError> {
    #[cfg(windows)]
    let valid = crate::win::toast::validate_toast_xml(xml);
    #[cfg(not(windows))]
    let valid = {
        let _ = xml;
        Ok(())
    };
    valid
}
