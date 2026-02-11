use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Plotters(
        #[from]
        plotters::drawing::DrawingAreaErrorKind<
            <plotters::backend::BitMapBackend<'static> as plotters::backend::DrawingBackend>::ErrorType,
        >,
    ),
}

pub type AppResult<T> = Result<T, AppError>;

impl From<&'static str> for AppError {
    fn from(value: &'static str) -> Self {
        AppError::Message(value.to_owned())
    }
}

impl From<String> for AppError {
    fn from(value: String) -> Self {
        AppError::Message(value)
    }
}
