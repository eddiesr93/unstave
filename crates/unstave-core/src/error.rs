use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to walk {root}: {source}")]
    Walk {
        root: PathBuf,
        #[source]
        source: ignore::Error,
    },

    #[error("invalid config at {path}: {source}")]
    Config {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("invalid glob {pattern:?}: {source}")]
    Glob {
        pattern: String,
        #[source]
        source: globset::Error,
    },

    #[error("failed to build resolver for package {package}: {message}")]
    Resolver {
        package: PathBuf,
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    /// Stable, machine-readable name for this error variant.
    ///
    /// Kept deliberately stable so callers (e.g. the NAPI boundary) can surface a
    /// programmatic `code` to consumers without parsing the human-readable message.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Error::Io { .. } => "Io",
            Error::Walk { .. } => "Walk",
            Error::Config { .. } => "Config",
            Error::Glob { .. } => "Glob",
            Error::Resolver { .. } => "Resolver",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_name_is_stable_per_variant() {
        assert_eq!(
            Error::io(
                "a.ts",
                std::io::Error::new(std::io::ErrorKind::NotFound, "nope")
            )
            .variant_name(),
            "Io"
        );
        assert_eq!(
            Error::Resolver {
                package: PathBuf::from("."),
                message: "boom".to_string(),
                source: Box::new(std::io::Error::new(std::io::ErrorKind::Other, "source")),
            }
            .variant_name(),
            "Resolver"
        );
    }
}
