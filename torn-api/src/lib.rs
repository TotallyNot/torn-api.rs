use thiserror::Error;

pub mod executor;
#[cfg(feature = "models")]
pub mod models;
#[cfg(feature = "requests")]
pub mod parameters;
pub mod request;
#[cfg(feature = "scopes")]
pub mod scopes;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ApiError {
    #[error("Unhandled error, should not occur")]
    Unknown,
    #[error("Private key is empty in current request")]
    KeyIsEmpty,
    #[error("Private key is wrong/incorrect format")]
    IncorrectKey,
    #[error("Requesting an incorrect basic type")]
    WrongType,
    #[error("Requesting incorect selection fields")]
    WrongFields,
    #[error(
        "Requests are blocked for a small period of time because of too many requests per user"
    )]
    TooManyRequest,
    #[error("Wrong ID value")]
    IncorrectId,
    #[error("A requested selection is private")]
    IncorrectIdEntityRelation,
    #[error("Current IP is banned for a small period of time because of abuse")]
    IpBlock,
    #[error("Api system is currently disabled")]
    ApiDisabled,
    #[error("Current key can't be used because owner is in federal jail")]
    KeyOwnerInFederalJail,
    #[error("You can only change your API key once every 60 seconds")]
    KeyChange,
    #[error("Error reading key from Database")]
    KeyRead,
    #[error("The key owner hasn't been online for more than 7 days")]
    TemporaryInactivity,
    #[error("Too many records have been pulled today by this user from our cloud services")]
    DailyReadLimit,
    #[error("An error code specifically for testing purposes that has no dedicated meaning")]
    TemporaryError,
    #[error("A selection is being called of which this key does not have permission to access")]
    InsufficientAccessLevel,
    #[error("Backend error occurred, please try again")]
    Backend,
    #[error("API key has been paused by the owner")]
    Paused,
    #[error("Must be migrated to crimes 2.0")]
    NotMigratedCrimes,
    #[error("Race not yet finished")]
    RaceNotFinished,
    #[error("Wrong cat value")]
    IncorrectCategory,
    #[error("This selection is only available in API v1")]
    OnlyInV1,
    #[error("This selection is only available in API v2")]
    OnlyInV2,
    #[error("Closed temporarily")]
    ClosedTemporarily,
    #[error("Other: {message}")]
    Other { code: u16, message: String },
}

impl ApiError {
    pub fn new(code: u16, message: &str) -> Self {
        match code {
            0 => Self::Unknown,
            1 => Self::KeyIsEmpty,
            2 => Self::IncorrectKey,
            3 => Self::WrongType,
            4 => Self::WrongFields,
            5 => Self::TooManyRequest,
            6 => Self::IncorrectId,
            7 => Self::IncorrectIdEntityRelation,
            8 => Self::IpBlock,
            9 => Self::ApiDisabled,
            10 => Self::KeyOwnerInFederalJail,
            11 => Self::KeyChange,
            12 => Self::KeyRead,
            13 => Self::TemporaryInactivity,
            14 => Self::DailyReadLimit,
            15 => Self::TemporaryError,
            16 => Self::InsufficientAccessLevel,
            17 => Self::Backend,
            18 => Self::Paused,
            19 => Self::NotMigratedCrimes,
            20 => Self::RaceNotFinished,
            21 => Self::IncorrectCategory,
            22 => Self::OnlyInV1,
            23 => Self::OnlyInV2,
            24 => Self::ClosedTemporarily,
            other => Self::Other {
                code: other,
                message: message.to_owned(),
            },
        }
    }

    pub fn code(&self) -> u16 {
        match self {
            Self::Unknown => 0,
            Self::KeyIsEmpty => 1,
            Self::IncorrectKey => 2,
            Self::WrongType => 3,
            Self::WrongFields => 4,
            Self::TooManyRequest => 5,
            Self::IncorrectId => 6,
            Self::IncorrectIdEntityRelation => 7,
            Self::IpBlock => 8,
            Self::ApiDisabled => 9,
            Self::KeyOwnerInFederalJail => 10,
            Self::KeyChange => 11,
            Self::KeyRead => 12,
            Self::TemporaryInactivity => 13,
            Self::DailyReadLimit => 14,
            Self::TemporaryError => 15,
            Self::InsufficientAccessLevel => 16,
            Self::Backend => 17,
            Self::Paused => 18,
            Self::NotMigratedCrimes => 19,
            Self::RaceNotFinished => 20,
            Self::IncorrectCategory => 21,
            Self::OnlyInV1 => 22,
            Self::OnlyInV2 => 23,
            Self::ClosedTemporarily => 24,
            Self::Other { code, .. } => *code,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParameterError {
    #[error("value `{value}` is out of range for parameter {name}")]
    OutOfRange { name: &'static str, value: i32 },
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Parameter error: {0}")]
    Parameter(#[from] ParameterError),
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Parsing error: {0}")]
    Parsing(#[from] serde_json::Error),
    #[error("Api error: {0}")]
    Api(#[from] ApiError),
}
