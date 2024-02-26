#[derive(Debug)]
pub enum Error {
    Anticipated(String, i32, Option<Box<dyn std::error::Error>>),
    Unexpected(Box<dyn std::error::Error>),
}

pub fn anticipated(message: &str, status: i32) -> Error {
    Error::Anticipated(message.to_string(), status, None)
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self {
            Error::Anticipated(msg, _, _) => {
                write!(f, "{}", &msg)
            }
            Error::Unexpected(e) => {
                write!(f, "{}", &e)
            }
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Unexpected(Box::new(e))
    }
}
