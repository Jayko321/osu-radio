use std::ffi::OsString;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LaunchMode {
    Songs,
    ComponentGallery,
    Help,
}

impl LaunchMode {
    pub(crate) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut mode = Self::Songs;
        for arg in args {
            match arg.to_str() {
                Some("--component-gallery") if mode == Self::Songs => {
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
    fn default_gallery_help_and_invalid_arguments() {
        assert_eq!(LaunchMode::parse([]), Ok(LaunchMode::Songs));
        assert_eq!(
            LaunchMode::parse(["--component-gallery".into()]),
            Ok(LaunchMode::ComponentGallery)
        );
        for help in ["--help", "-h"] {
            assert_eq!(LaunchMode::parse([help.into()]), Ok(LaunchMode::Help));
        }
        assert!(
            LaunchMode::parse(["--component-gallery".into(), "--component-gallery".into()])
                .is_err()
        );
        assert!(LaunchMode::parse(["--unknown".into()]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_argument_reports_an_error() {
        use std::os::unix::ffi::OsStringExt;
        assert!(LaunchMode::parse([OsString::from_vec(vec![255])]).is_err());
    }
}
