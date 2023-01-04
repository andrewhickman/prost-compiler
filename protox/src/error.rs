use std::{fmt, io, path::PathBuf};

use miette::{Diagnostic, MietteError, NamedSource, SourceCode, SourceSpan};
use prost_reflect::DescriptorError;
use protox_parse::ParseError;
use thiserror::Error;

/// An error that can occur when compiling protobuf files.
#[derive(Debug, Diagnostic, Error)]
#[error(transparent)]
#[diagnostic(transparent)]
pub struct Error {
    kind: Box<ErrorKind>,
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum ErrorKind {
    #[error("{}", err)]
    #[diagnostic(forward(err))]
    Parse { err: ParseError },
    #[error("{}", err)]
    #[diagnostic(forward(err))]
    Check { err: DescriptorError },
    #[error("error opening file '{path}'")]
    OpenFile {
        path: PathBuf,
        #[source]
        err: io::Error,
        #[source_code]
        src: DynSourceCode,
        #[label("imported here")]
        span: Option<SourceSpan>,
    },
    #[error("file is too large")]
    #[diagnostic(help("the maximum file length is 2,147,483,647 bytes"))]
    FileTooLarge {
        #[source_code]
        src: DynSourceCode,
        #[label("imported here")]
        span: Option<SourceSpan>,
    },
    #[error("import '{name}' not found")]
    ImportNotFound {
        name: String,
        #[source_code]
        src: DynSourceCode,
        #[label("imported here")]
        span: Option<SourceSpan>,
    },
    #[error("import cycle detected: {cycle}")]
    CircularImport { cycle: String },
    #[error("file '{path}' is not in any include path")]
    FileNotIncluded { path: PathBuf },
    #[error("path '{path}' is shadowed by '{shadow}' in the include paths")]
    #[diagnostic(help("either pass '{}' as the input file, or re-order the include paths so that '{}' comes first", shadow.display(), path.display()))]
    FileShadowed { path: PathBuf, shadow: PathBuf },
    #[error(transparent)]
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Default)]
pub(crate) struct DynSourceCode(Option<Box<dyn SourceCode>>);

impl fmt::Debug for DynSourceCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("DynSourceCode").finish_non_exhaustive()
    }
}

impl SourceCode for DynSourceCode {
    fn read_span<'a>(
        &'a self,
        span: &miette::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, MietteError> {
        if let Some(src) = &self.0 {
            src.read_span(span, context_lines_before, context_lines_after)
        } else {
            Err(MietteError::OutOfBounds)
        }
    }
}

impl From<NamedSource> for DynSourceCode {
    fn from(source: NamedSource) -> Self {
        DynSourceCode(Some(Box::new(source)))
    }
}

impl Error {
    /// Creates an instance of [`struct@Error`] with an arbitrary payload.
    pub fn new<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Error::from_kind(ErrorKind::Custom(error.into()))
    }

    /// Creates an instance of [`struct@Error`] indicating that an imported file could not be found.
    ///
    /// This error should be returned by [`FileResolver`](crate::file::FileResolver) instances if a file is not found.
    pub fn file_not_found(name: &str) -> Self {
        Error::from_kind(ErrorKind::ImportNotFound {
            name: name.to_owned(),
            src: DynSourceCode::default(),
            span: None,
        })
    }

    pub(crate) fn from_kind(kind: ErrorKind) -> Self {
        Error {
            kind: Box::new(kind),
        }
    }

    #[cfg(test)]
    pub(crate) fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// Returns true if this is an instance of [`Error::file_not_found()`]
    pub fn is_file_not_found(&self) -> bool {
        match &*self.kind {
            ErrorKind::ImportNotFound { .. } => true,
            ErrorKind::OpenFile { err, .. } => err.kind() == io::ErrorKind::NotFound,
            _ => false,
        }
    }
}

impl From<ParseError> for Error {
    fn from(err: ParseError) -> Self {
        Error::from_kind(ErrorKind::Parse { err })
    }
}
