use std::backtrace::{Backtrace, BacktraceStatus};
use std::borrow::Cow;
use std::fmt::{Debug, Display, Formatter};
use std::panic::Location;
use betrayer::{ErrorSource, TrayError};

use windows::core::{Error, HRESULT};
use windows::Win32::Foundation::NO_ERROR;

pub type Result<T> = std::result::Result<T, TracedError>;

pub enum Trace {
    Backtrace(Box<Backtrace>),
    Location(&'static Location<'static>)
}

impl Trace {
    #[track_caller]
    pub fn capture() -> Self {
        let capture = Backtrace::capture();
        match capture.status() {
            BacktraceStatus::Captured => Self::Backtrace(Box::new(capture)),
            _ => Self::Location(Location::caller())
        }
    }

    pub fn is_backtrace(&self) -> bool {
        matches!(self, Self::Backtrace(_))
    }
}

impl Debug for Trace {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Trace::Backtrace(capture) => Debug::fmt(capture, f),
            Trace::Location(location) => Debug::fmt(location, f)
        }
    }
}

impl Display for Trace {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Trace::Backtrace(capture) => Display::fmt(capture, f),
            Trace::Location(location) => Display::fmt(location, f)
        }
    }
}

enum InnerError {
    Win(Error),
    String(Cow<'static, str>)
}

pub struct TracedError {
    inner: InnerError,
    backtrace: Trace
}

impl TracedError {
    pub fn message(&self) -> String {
        match &self.inner {
            InnerError::Win(err) => err.message().to_string_lossy(),
            InnerError::String(msg) => msg.clone().into_owned()
        }
    }

    pub fn trace(&self) -> &Trace {
        &self.backtrace
    }
}

impl Debug for TracedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Error");

        match &self.inner {
            InnerError::Win(err) => debug
                .field("code", &err.code())
                .field("message", &err.message()),
            InnerError::String(msg) => debug
                .field("message", &FromDisplay(msg))
        };

        if !self.backtrace.is_backtrace() {
            debug.field("location", &FromDisplay(&self.backtrace));
        }

        debug.finish()?;
        if self.backtrace.is_backtrace() {
            write!(f, "\n{}", self.backtrace)?;
        }

        Ok(())
    }
}

impl Display for TracedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            InnerError::Win(inner) => Display::fmt(inner, f),
            InnerError::String(inner) => Display::fmt(inner, f)
        }
    }
}

impl std::error::Error for TracedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.inner {
            InnerError::Win(err) => Some(err),
            InnerError::String(_) => None
        }
    }
}

impl From<Error> for TracedError {
    #[track_caller]
    fn from(value: Error) -> Self {
        Self {
            inner: InnerError::Win(value),
            backtrace: Trace::capture()
        }
    }
}

impl From<TrayError> for TracedError {
    #[track_caller]
    fn from(value: TrayError) -> Self {
        let inner = match value.source() {
            ErrorSource::Os(err) => {
                let code = HRESULT(err.code().0);
                InnerError::Win(Error::from(code))
            },
            ErrorSource::Custom(inner) => InnerError::String(inner.clone())
        };
        Self {
            inner,
            backtrace: Trace::Location(value.location())
        }
    }
}

struct FromDisplay<T>(pub T);

impl<T: Display> Debug for FromDisplay<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub trait ResultEx<T> {
    fn to_win_result(self) -> windows::core::Result<T>;
}

impl<T> ResultEx<T> for Result<T> {
    fn to_win_result(self) -> windows::core::Result<T> {
        self.map_err(|err| match err.inner {
            InnerError::Win(err) => err,
            _ => Error::from(NO_ERROR)
        })
    }
}

pub trait OptionExt<T> {
    fn some(self) -> windows::core::Result<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn some(self) -> windows::core::Result<T> {
        self.ok_or(Error::from(NO_ERROR))
    }
}

#[macro_export]
macro_rules! win_assert {
    ($cond:expr) => {
        if !($cond) {
            Err(windows::core::Error::from(windows::Win32::Foundation::ERROR_ASSERTION_FAILURE))?;
        }
    };
}
