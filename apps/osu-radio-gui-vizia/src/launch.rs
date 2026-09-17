use std::ffi::OsString;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LaunchMode {
    Player,
    ComponentGallery,
    Help,
}

impl LaunchMode {
    pub(crate) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut mode = Self::Player;
        for arg in args {
            match arg.to_str() {
                Some("--component-gallery") if mode == Self::Player => {
                    mode = Self::ComponentGallery;
                }
                Some("--help" | "-h") => return Ok(Self::Help),
                _ => {
                    return Err(format!(
                        "Unknown or repeated argument: {}",
                        arg.to_string_lossy()
                    ));
                }
            }
        }
        Ok(mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launch_modes_are_explicit_and_reject_invalid_arguments() {
        assert_eq!(LaunchMode::parse([]), Ok(LaunchMode::Player));
        assert_eq!(
            LaunchMode::parse(["--component-gallery".into()]),
            Ok(LaunchMode::ComponentGallery)
        );
        assert_eq!(LaunchMode::parse(["--help".into()]), Ok(LaunchMode::Help));
        assert!(
            LaunchMode::parse(["--component-gallery".into(), "--component-gallery".into()])
                .is_err()
        );
        assert!(LaunchMode::parse(["--unknown".into()]).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn non_unicode_arguments_are_errors_without_panicking() {
        use std::os::unix::ffi::OsStringExt;
        assert!(LaunchMode::parse([OsString::from_vec(vec![255])]).is_err());
    }
}
