use crate::model::{GeneratedCase, ResponsePayload};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    Success,
    Failure(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub check_name: &'static str,
    pub status: CheckStatus,
}

impl CheckResult {
    pub fn success(check_name: &'static str) -> Self {
        Self {
            check_name,
            status: CheckStatus::Success,
        }
    }

    pub fn failure(check_name: &'static str, message: impl Into<String>) -> Self {
        Self {
            check_name,
            status: CheckStatus::Failure(message.into()),
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self.status, CheckStatus::Success)
    }
}

pub trait Check: Send + Sync {
    fn name(&self) -> &'static str;
    fn evaluate(&self, case: &GeneratedCase, response: &ResponsePayload) -> CheckResult;
}
