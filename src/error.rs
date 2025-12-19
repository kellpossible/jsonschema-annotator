use std::borrow::Cow;

/// A generic error type with context chaining and hidden source errors.
#[derive(Debug)]
pub struct Error<K> {
    pub kind: K,
    pub(crate) context: Vec<Cow<'static, str>>,
    pub(crate) source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

impl<K> Error<K> {
    pub fn new(kind: K) -> Self {
        Self {
            kind,
            context: Vec::new(),
            source: None,
        }
    }

    pub fn map_kind<NK, F>(self, mapper: F) -> Error<NK>
    where
        F: Fn(K) -> NK,
    {
        Error {
            kind: mapper(self.kind),
            context: self.context,
            source: self.source,
        }
    }

    pub fn add_context(mut self, context: impl Into<Cow<'static, str>>) -> Self {
        self.context.push(context.into());
        self
    }

    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn with_boxed_source(
        mut self,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    ) -> Self {
        self.source = Some(source);
        self
    }
}

impl<K> From<K> for Error<K> {
    fn from(value: K) -> Self {
        Error::new(value)
    }
}

pub trait ResultExt<T, K> {
    fn add_context(self, context: impl Into<Cow<'static, str>>) -> Result<T, Error<K>>;
    fn add_context_fn<C: Into<Cow<'static, str>>, F: FnOnce() -> C>(
        self,
        context: F,
    ) -> Result<T, Error<K>>;
}

impl<T, K> ResultExt<T, K> for Result<T, Error<K>> {
    fn add_context(self, context: impl Into<Cow<'static, str>>) -> Result<T, Error<K>> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(error.add_context(context)),
        }
    }

    fn add_context_fn<C: Into<Cow<'static, str>>, F: FnOnce() -> C>(
        self,
        context: F,
    ) -> Result<T, Error<K>> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(error.add_context(context().into())),
        }
    }
}

impl<K> std::fmt::Display for Error<K>
where
    K: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.kind.fmt(f)?;

        if !self.context.is_empty() {
            write!(f, " context: [")?;
            for (i, context) in self.context.iter().rev().enumerate() {
                write!(f, "{}", context)?;
                if i < self.context.len() - 1 {
                    write!(f, ", ")?;
                }
            }
            write!(f, "]")?;
        }

        Ok(())
    }
}

impl<K> std::error::Error for Error<K>
where
    K: std::fmt::Display + std::fmt::Debug,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|s| s.as_ref() as _)
    }
}

// Error kinds for schema operations
#[derive(Debug)]
pub enum SchemaErrorKind {
    Io,
    ValueParse,
    InvalidSchema,
    RefResolution,
}

impl std::fmt::Display for SchemaErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaErrorKind::Io => write!(f, "I/O error"),
            SchemaErrorKind::ValueParse => write!(f, "failed to parse value"),
            SchemaErrorKind::InvalidSchema => write!(f, "invalid schema"),
            SchemaErrorKind::RefResolution => write!(f, "failed to resolve $ref"),
        }
    }
}

// Error kinds for annotator operations
#[derive(Debug)]
pub enum AnnotatorErrorKind {
    Parse,
    Io,
}

impl std::fmt::Display for AnnotatorErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnnotatorErrorKind::Parse => write!(f, "failed to parse target document"),
            AnnotatorErrorKind::Io => write!(f, "I/O error"),
        }
    }
}

pub type SchemaError = Error<SchemaErrorKind>;
pub type AnnotatorError = Error<AnnotatorErrorKind>;
