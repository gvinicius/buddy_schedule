use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleRole {
    Admin,
    User,
}

impl ScheduleRole {
    pub fn as_str(self) -> &'static str {
        match self {
            ScheduleRole::Admin => "admin",
            ScheduleRole::User => "user",
        }
    }
}

impl TryFrom<&str> for ScheduleRole {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "admin" => Ok(ScheduleRole::Admin),
            "user" => Ok(ScheduleRole::User),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Morning,
    Afternoon,
    Night,
    Sleep,
}

impl Period {
    pub fn as_str(self) -> &'static str {
        match self {
            Period::Morning => "morning",
            Period::Afternoon => "afternoon",
            Period::Night => "night",
            Period::Sleep => "sleep",
        }
    }
}

impl TryFrom<&str> for Period {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "morning" => Ok(Period::Morning),
            "afternoon" => Ok(Period::Afternoon),
            "night" => Ok(Period::Night),
            "sleep" => Ok(Period::Sleep),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub is_superadmin: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Schedule {
    pub id: Uuid,
    pub name: String,
    pub subject_type: String,
    pub subject_name: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScheduleWithRole {
    pub schedule: Schedule,
    pub role: ScheduleRole,
}

#[derive(Clone, Debug, Serialize)]
pub struct Shift {
    pub id: Uuid,
    pub schedule_id: Uuid,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub period: Period,
    pub assigned_user_id: Option<Uuid>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ShiftComment {
    pub id: Uuid,
    pub shift_id: Uuid,
    pub user_id: Uuid,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RotationTemplate {
    pub id: Uuid,
    pub schedule_id: Uuid,
    pub name: String,
    pub definition: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}
