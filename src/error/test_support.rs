use super::{
    ConfigError, DistributedError, MetricsError, ScriptError, ServiceError, SinkError,
    ValidationError,
};

impl From<&'static str> for ValidationError {
    fn from(message: &'static str) -> Self {
        ValidationError::TestExpectation { message }
    }
}

impl From<String> for ValidationError {
    fn from(value: String) -> Self {
        ValidationError::TestExpectationValue {
            message: "Test expectation failed",
            value,
        }
    }
}

impl From<&'static str> for ConfigError {
    fn from(message: &'static str) -> Self {
        ConfigError::TestExpectation { message }
    }
}

impl From<String> for ConfigError {
    fn from(value: String) -> Self {
        ConfigError::TestExpectationValue {
            message: "Test expectation failed",
            value,
        }
    }
}

impl From<&'static str> for MetricsError {
    fn from(message: &'static str) -> Self {
        MetricsError::TestExpectation { message }
    }
}

impl From<String> for MetricsError {
    fn from(value: String) -> Self {
        MetricsError::TestExpectationValue {
            message: "Test expectation failed",
            value,
        }
    }
}

impl From<&'static str> for DistributedError {
    fn from(message: &'static str) -> Self {
        DistributedError::TestExpectation { message }
    }
}

impl From<String> for DistributedError {
    fn from(value: String) -> Self {
        DistributedError::TestExpectationValue {
            message: "Test expectation failed",
            value,
        }
    }
}

impl From<&'static str> for ScriptError {
    fn from(message: &'static str) -> Self {
        ScriptError::TestExpectation { message }
    }
}

impl From<String> for ScriptError {
    fn from(value: String) -> Self {
        ScriptError::TestExpectationValue {
            message: "Test expectation failed",
            value,
        }
    }
}

impl From<&'static str> for ServiceError {
    fn from(message: &'static str) -> Self {
        ServiceError::TestExpectation { message }
    }
}

impl From<String> for ServiceError {
    fn from(value: String) -> Self {
        ServiceError::TestExpectationValue {
            message: "Test expectation failed",
            value,
        }
    }
}

impl From<&'static str> for SinkError {
    fn from(message: &'static str) -> Self {
        SinkError::TestExpectation { message }
    }
}

impl From<String> for SinkError {
    fn from(value: String) -> Self {
        SinkError::TestExpectationValue {
            message: "Test expectation failed",
            value,
        }
    }
}
