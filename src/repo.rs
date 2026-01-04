use crate::{
    error::{AppError, AppResult},
    models::{
        Period, RotationTemplate, Schedule, ScheduleRole, ScheduleWithRole, Shift, ShiftComment,
        User,
    },
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct NewUser {
    pub email: String,
    pub password_hash: String,
    pub is_superadmin: bool,
}

#[derive(Clone, Debug)]
pub struct NewSchedule {
    pub name: String,
    pub subject_type: String,
    pub subject_name: String,
    pub created_by: Uuid,
}

#[derive(Clone, Debug)]
pub struct NewShift {
    pub schedule_id: Uuid,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub period: Period,
    pub created_by: Uuid,
}

#[derive(Clone, Debug)]
pub struct NewShiftComment {
    pub shift_id: Uuid,
    pub user_id: Uuid,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct NewTemplate {
    pub schedule_id: Uuid,
    pub name: String,
    pub definition: serde_json::Value,
    pub created_by: Uuid,
}

#[async_trait]
pub trait Repo: Send + Sync {
    async fn count_users(&self) -> AppResult<i64>;
    async fn create_user(&self, nu: NewUser) -> AppResult<User>;
    async fn find_user_by_email(&self, email: &str) -> AppResult<Option<(User, String)>>;
    async fn get_user(&self, user_id: Uuid) -> AppResult<Option<User>>;

    async fn create_schedule(&self, ns: NewSchedule) -> AppResult<Schedule>;
    async fn list_schedules_for_user(&self, user_id: Uuid) -> AppResult<Vec<ScheduleWithRole>>;
    async fn get_schedule(&self, schedule_id: Uuid) -> AppResult<Option<Schedule>>;
    async fn get_schedule_role(
        &self,
        schedule_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<Option<ScheduleRole>>;
    async fn list_schedule_members(
        &self,
        schedule_id: Uuid,
    ) -> AppResult<Vec<(User, ScheduleRole)>>;
    async fn add_member(
        &self,
        schedule_id: Uuid,
        user_id: Uuid,
        role: ScheduleRole,
    ) -> AppResult<()>;
    async fn set_member_role(
        &self,
        schedule_id: Uuid,
        user_id: Uuid,
        role: ScheduleRole,
    ) -> AppResult<()>;

    async fn create_shift(&self, ns: NewShift) -> AppResult<Shift>;
    async fn list_shifts(
        &self,
        schedule_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> AppResult<Vec<Shift>>;
    async fn get_shift(&self, shift_id: Uuid) -> AppResult<Option<Shift>>;
    async fn assign_shift(&self, shift_id: Uuid, assigned_user_id: Option<Uuid>) -> AppResult<()>;

    async fn add_shift_comment(&self, nc: NewShiftComment) -> AppResult<ShiftComment>;
    async fn list_shift_comments(&self, shift_id: Uuid) -> AppResult<Vec<ShiftComment>>;

    async fn create_template(&self, nt: NewTemplate) -> AppResult<RotationTemplate>;
    async fn list_templates(&self, schedule_id: Uuid) -> AppResult<Vec<RotationTemplate>>;
    async fn get_template(&self, template_id: Uuid) -> AppResult<Option<RotationTemplate>>;
}

pub struct PgRepo {
    pool: PgPool,
}

impl PgRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Repo for PgRepo {
    async fn count_users(&self) -> AppResult<i64> {
        let row = sqlx::query("select count(*)::bigint as c from app_user")
            .fetch_one(&self.pool)
            .await
            .map_err(|_| AppError::Internal)?;
        Ok(row.get::<i64, _>("c"))
    }

    async fn create_user(&self, nu: NewUser) -> AppResult<User> {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            insert into app_user (id, email, password_hash, is_superadmin)
            values ($1, $2, $3, $4)
            returning id, email, is_superadmin, created_at
            "#,
        )
        .bind(id)
        .bind(&nu.email)
        .bind(&nu.password_hash)
        .bind(nu.is_superadmin)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let Some(db) = e.as_database_error() {
                if db.is_unique_violation() {
                    return AppError::Conflict("email already exists".to_string());
                }
            }
            AppError::Internal
        })?;

        Ok(User {
            id: row.get("id"),
            email: row.get("email"),
            is_superadmin: row.get("is_superadmin"),
            created_at: row.get("created_at"),
        })
    }

    async fn find_user_by_email(&self, email: &str) -> AppResult<Option<(User, String)>> {
        let row = sqlx::query(
            r#"
            select id, email, password_hash, is_superadmin, created_at
            from app_user
            where email = $1
            "#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        Ok(row.map(|r| {
            let u = User {
                id: r.get("id"),
                email: r.get("email"),
                is_superadmin: r.get("is_superadmin"),
                created_at: r.get("created_at"),
            };
            let ph: String = r.get("password_hash");
            (u, ph)
        }))
    }

    async fn get_user(&self, user_id: Uuid) -> AppResult<Option<User>> {
        let row =
            sqlx::query("select id, email, is_superadmin, created_at from app_user where id = $1")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| AppError::Internal)?;

        Ok(row.map(|r| User {
            id: r.get("id"),
            email: r.get("email"),
            is_superadmin: r.get("is_superadmin"),
            created_at: r.get("created_at"),
        }))
    }

    async fn create_schedule(&self, ns: NewSchedule) -> AppResult<Schedule> {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            insert into schedule (id, name, subject_type, subject_name, created_by)
            values ($1, $2, $3, $4, $5)
            returning id, name, subject_type, subject_name, created_by, created_at
            "#,
        )
        .bind(id)
        .bind(&ns.name)
        .bind(&ns.subject_type)
        .bind(&ns.subject_name)
        .bind(ns.created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        // creator becomes admin member
        sqlx::query(
            "insert into schedule_member (schedule_id, user_id, role) values ($1, $2, 'admin')",
        )
        .bind(id)
        .bind(ns.created_by)
        .execute(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        Ok(Schedule {
            id: row.get("id"),
            name: row.get("name"),
            subject_type: row.get("subject_type"),
            subject_name: row.get("subject_name"),
            created_by: row.get("created_by"),
            created_at: row.get("created_at"),
        })
    }

    async fn list_schedules_for_user(&self, user_id: Uuid) -> AppResult<Vec<ScheduleWithRole>> {
        let rows = sqlx::query(
            r#"
            select s.id, s.name, s.subject_type, s.subject_name, s.created_by, s.created_at, sm.role
            from schedule s
            join schedule_member sm on sm.schedule_id = s.id
            where sm.user_id = $1
            order by s.created_at desc
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let role_str: String = r.get("role");
            let role = ScheduleRole::try_from(role_str.as_str()).map_err(|_| AppError::Internal)?;
            out.push(ScheduleWithRole {
                schedule: Schedule {
                    id: r.get("id"),
                    name: r.get("name"),
                    subject_type: r.get("subject_type"),
                    subject_name: r.get("subject_name"),
                    created_by: r.get("created_by"),
                    created_at: r.get("created_at"),
                },
                role,
            });
        }
        Ok(out)
    }

    async fn get_schedule(&self, schedule_id: Uuid) -> AppResult<Option<Schedule>> {
        let row = sqlx::query(
            "select id, name, subject_type, subject_name, created_by, created_at from schedule where id = $1",
        )
        .bind(schedule_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        Ok(row.map(|r| Schedule {
            id: r.get("id"),
            name: r.get("name"),
            subject_type: r.get("subject_type"),
            subject_name: r.get("subject_name"),
            created_by: r.get("created_by"),
            created_at: r.get("created_at"),
        }))
    }

    async fn get_schedule_role(
        &self,
        schedule_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<Option<ScheduleRole>> {
        let row =
            sqlx::query("select role from schedule_member where schedule_id = $1 and user_id = $2")
                .bind(schedule_id)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| AppError::Internal)?;

        Ok(match row {
            None => None,
            Some(r) => {
                let role_str: String = r.get("role");
                Some(ScheduleRole::try_from(role_str.as_str()).map_err(|_| AppError::Internal)?)
            }
        })
    }

    async fn list_schedule_members(
        &self,
        schedule_id: Uuid,
    ) -> AppResult<Vec<(User, ScheduleRole)>> {
        let rows = sqlx::query(
            r#"
            select u.id, u.email, u.is_superadmin, u.created_at, sm.role
            from schedule_member sm
            join app_user u on u.id = sm.user_id
            where sm.schedule_id = $1
            order by sm.created_at
            "#,
        )
        .bind(schedule_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let role_str: String = r.get("role");
            let role = ScheduleRole::try_from(role_str.as_str()).map_err(|_| AppError::Internal)?;
            out.push((
                User {
                    id: r.get("id"),
                    email: r.get("email"),
                    is_superadmin: r.get("is_superadmin"),
                    created_at: r.get("created_at"),
                },
                role,
            ));
        }
        Ok(out)
    }

    async fn add_member(
        &self,
        schedule_id: Uuid,
        user_id: Uuid,
        role: ScheduleRole,
    ) -> AppResult<()> {
        let role = role.as_str();
        sqlx::query("insert into schedule_member (schedule_id, user_id, role) values ($1, $2, $3)")
            .bind(schedule_id)
            .bind(user_id)
            .bind(role)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                if let Some(db) = e.as_database_error() {
                    if db.is_unique_violation() {
                        return AppError::Conflict("user already in schedule".to_string());
                    }
                }
                AppError::Internal
            })?;
        Ok(())
    }

    async fn set_member_role(
        &self,
        schedule_id: Uuid,
        user_id: Uuid,
        role: ScheduleRole,
    ) -> AppResult<()> {
        let role = role.as_str();
        let res = sqlx::query(
            "update schedule_member set role = $3 where schedule_id = $1 and user_id = $2",
        )
        .bind(schedule_id)
        .bind(user_id)
        .bind(role)
        .execute(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    async fn create_shift(&self, ns: NewShift) -> AppResult<Shift> {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            insert into shift (id, schedule_id, starts_at, ends_at, period, created_by)
            values ($1, $2, $3, $4, $5, $6)
            returning id, schedule_id, starts_at, ends_at, period, assigned_user_id, created_by, created_at
            "#,
        )
        .bind(id)
        .bind(ns.schedule_id)
        .bind(ns.starts_at)
        .bind(ns.ends_at)
        .bind(ns.period.as_str())
        .bind(ns.created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        let period_str: String = row.get("period");
        let period = Period::try_from(period_str.as_str()).map_err(|_| AppError::Internal)?;
        Ok(Shift {
            id: row.get("id"),
            schedule_id: row.get("schedule_id"),
            starts_at: row.get("starts_at"),
            ends_at: row.get("ends_at"),
            period,
            assigned_user_id: row.get("assigned_user_id"),
            created_by: row.get("created_by"),
            created_at: row.get("created_at"),
        })
    }

    async fn list_shifts(
        &self,
        schedule_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> AppResult<Vec<Shift>> {
        let rows = sqlx::query(
            r#"
            select id, schedule_id, starts_at, ends_at, period, assigned_user_id, created_by, created_at
            from shift
            where schedule_id = $1 and starts_at >= $2 and starts_at < $3
            order by starts_at asc
            "#,
        )
        .bind(schedule_id)
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let period_str: String = r.get("period");
            let period = Period::try_from(period_str.as_str()).map_err(|_| AppError::Internal)?;
            out.push(Shift {
                id: r.get("id"),
                schedule_id: r.get("schedule_id"),
                starts_at: r.get("starts_at"),
                ends_at: r.get("ends_at"),
                period,
                assigned_user_id: r.get("assigned_user_id"),
                created_by: r.get("created_by"),
                created_at: r.get("created_at"),
            });
        }
        Ok(out)
    }

    async fn get_shift(&self, shift_id: Uuid) -> AppResult<Option<Shift>> {
        let row = sqlx::query(
            "select id, schedule_id, starts_at, ends_at, period, assigned_user_id, created_by, created_at from shift where id = $1",
        )
        .bind(shift_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        Ok(row.map(|r| {
            let period_str: String = r.get("period");
            let period = Period::try_from(period_str.as_str()).unwrap_or(Period::Morning);
            Shift {
                id: r.get("id"),
                schedule_id: r.get("schedule_id"),
                starts_at: r.get("starts_at"),
                ends_at: r.get("ends_at"),
                period,
                assigned_user_id: r.get("assigned_user_id"),
                created_by: r.get("created_by"),
                created_at: r.get("created_at"),
            }
        }))
    }

    async fn assign_shift(&self, shift_id: Uuid, assigned_user_id: Option<Uuid>) -> AppResult<()> {
        let res = sqlx::query("update shift set assigned_user_id = $2 where id = $1")
            .bind(shift_id)
            .bind(assigned_user_id)
            .execute(&self.pool)
            .await
            .map_err(|_| AppError::Internal)?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    async fn add_shift_comment(&self, nc: NewShiftComment) -> AppResult<ShiftComment> {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            insert into shift_comment (id, shift_id, user_id, body)
            values ($1, $2, $3, $4)
            returning id, shift_id, user_id, body, created_at
            "#,
        )
        .bind(id)
        .bind(nc.shift_id)
        .bind(nc.user_id)
        .bind(nc.body)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        Ok(ShiftComment {
            id: row.get("id"),
            shift_id: row.get("shift_id"),
            user_id: row.get("user_id"),
            body: row.get("body"),
            created_at: row.get("created_at"),
        })
    }

    async fn list_shift_comments(&self, shift_id: Uuid) -> AppResult<Vec<ShiftComment>> {
        let rows = sqlx::query(
            "select id, shift_id, user_id, body, created_at from shift_comment where shift_id = $1 order by created_at asc",
        )
        .bind(shift_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        Ok(rows
            .into_iter()
            .map(|r| ShiftComment {
                id: r.get("id"),
                shift_id: r.get("shift_id"),
                user_id: r.get("user_id"),
                body: r.get("body"),
                created_at: r.get("created_at"),
            })
            .collect())
    }

    async fn create_template(&self, nt: NewTemplate) -> AppResult<RotationTemplate> {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            insert into rotation_template (id, schedule_id, name, definition, created_by)
            values ($1, $2, $3, $4, $5)
            returning id, schedule_id, name, definition, created_by, created_at
            "#,
        )
        .bind(id)
        .bind(nt.schedule_id)
        .bind(nt.name)
        .bind(nt.definition)
        .bind(nt.created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        Ok(RotationTemplate {
            id: row.get("id"),
            schedule_id: row.get("schedule_id"),
            name: row.get("name"),
            definition: row.get("definition"),
            created_by: row.get("created_by"),
            created_at: row.get("created_at"),
        })
    }

    async fn list_templates(&self, schedule_id: Uuid) -> AppResult<Vec<RotationTemplate>> {
        let rows = sqlx::query(
            "select id, schedule_id, name, definition, created_by, created_at from rotation_template where schedule_id = $1 order by created_at desc",
        )
        .bind(schedule_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        Ok(rows
            .into_iter()
            .map(|r| RotationTemplate {
                id: r.get("id"),
                schedule_id: r.get("schedule_id"),
                name: r.get("name"),
                definition: r.get("definition"),
                created_by: r.get("created_by"),
                created_at: r.get("created_at"),
            })
            .collect())
    }

    async fn get_template(&self, template_id: Uuid) -> AppResult<Option<RotationTemplate>> {
        let row = sqlx::query(
            "select id, schedule_id, name, definition, created_by, created_at from rotation_template where id = $1",
        )
        .bind(template_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| AppError::Internal)?;

        Ok(row.map(|r| RotationTemplate {
            id: r.get("id"),
            schedule_id: r.get("schedule_id"),
            name: r.get("name"),
            definition: r.get("definition"),
            created_by: r.get("created_by"),
            created_at: r.get("created_at"),
        }))
    }
}

#[derive(Default)]
struct MemState {
    users: HashMap<Uuid, (User, String)>,
    schedules: HashMap<Uuid, Schedule>,
    members: HashMap<(Uuid, Uuid), ScheduleRole>,
    shifts: HashMap<Uuid, Shift>,
    comments: HashMap<Uuid, Vec<ShiftComment>>,
    templates: HashMap<Uuid, RotationTemplate>,
}

#[derive(Clone, Default)]
pub struct MemRepo {
    state: Arc<RwLock<MemState>>,
}

impl MemRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Repo for MemRepo {
    async fn count_users(&self) -> AppResult<i64> {
        Ok(self.state.read().unwrap().users.len() as i64)
    }

    async fn create_user(&self, nu: NewUser) -> AppResult<User> {
        let mut s = self.state.write().unwrap();
        if s.users.values().any(|(u, _)| u.email == nu.email) {
            return Err(AppError::Conflict("email already exists".to_string()));
        }
        let id = Uuid::new_v4();
        let user = User {
            id,
            email: nu.email,
            is_superadmin: nu.is_superadmin,
            created_at: Utc::now(),
        };
        s.users.insert(id, (user.clone(), nu.password_hash));
        Ok(user)
    }

    async fn find_user_by_email(&self, email: &str) -> AppResult<Option<(User, String)>> {
        let s = self.state.read().unwrap();
        Ok(s.users
            .values()
            .find(|(u, _)| u.email == email)
            .map(|(u, ph)| (u.clone(), ph.clone())))
    }

    async fn get_user(&self, user_id: Uuid) -> AppResult<Option<User>> {
        Ok(self
            .state
            .read()
            .unwrap()
            .users
            .get(&user_id)
            .map(|(u, _)| u.clone()))
    }

    async fn create_schedule(&self, ns: NewSchedule) -> AppResult<Schedule> {
        let mut s = self.state.write().unwrap();
        let id = Uuid::new_v4();
        let schedule = Schedule {
            id,
            name: ns.name,
            subject_type: ns.subject_type,
            subject_name: ns.subject_name,
            created_by: ns.created_by,
            created_at: Utc::now(),
        };
        s.schedules.insert(id, schedule.clone());
        s.members.insert((id, ns.created_by), ScheduleRole::Admin);
        Ok(schedule)
    }

    async fn list_schedules_for_user(&self, user_id: Uuid) -> AppResult<Vec<ScheduleWithRole>> {
        let s = self.state.read().unwrap();
        let mut out = Vec::new();
        for ((schedule_id, uid), role) in s.members.iter() {
            if *uid != user_id {
                continue;
            }
            if let Some(schedule) = s.schedules.get(schedule_id) {
                out.push(ScheduleWithRole {
                    schedule: schedule.clone(),
                    role: *role,
                });
            }
        }
        out.sort_by_key(|x| x.schedule.created_at);
        out.reverse();
        Ok(out)
    }

    async fn get_schedule(&self, schedule_id: Uuid) -> AppResult<Option<Schedule>> {
        Ok(self
            .state
            .read()
            .unwrap()
            .schedules
            .get(&schedule_id)
            .cloned())
    }

    async fn get_schedule_role(
        &self,
        schedule_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<Option<ScheduleRole>> {
        Ok(self
            .state
            .read()
            .unwrap()
            .members
            .get(&(schedule_id, user_id))
            .copied())
    }

    async fn list_schedule_members(
        &self,
        schedule_id: Uuid,
    ) -> AppResult<Vec<(User, ScheduleRole)>> {
        let s = self.state.read().unwrap();
        let mut out = Vec::new();
        for ((sid, uid), role) in s.members.iter() {
            if *sid != schedule_id {
                continue;
            }
            if let Some((user, _)) = s.users.get(uid) {
                out.push((user.clone(), *role));
            }
        }
        Ok(out)
    }

    async fn add_member(
        &self,
        schedule_id: Uuid,
        user_id: Uuid,
        role: ScheduleRole,
    ) -> AppResult<()> {
        let mut s = self.state.write().unwrap();
        let key = (schedule_id, user_id);
        if s.members.contains_key(&key) {
            return Err(AppError::Conflict("user already in schedule".to_string()));
        }
        s.members.insert(key, role);
        Ok(())
    }

    async fn set_member_role(
        &self,
        schedule_id: Uuid,
        user_id: Uuid,
        role: ScheduleRole,
    ) -> AppResult<()> {
        let mut s = self.state.write().unwrap();
        let key = (schedule_id, user_id);
        if !s.members.contains_key(&key) {
            return Err(AppError::NotFound);
        }
        s.members.insert(key, role);
        Ok(())
    }

    async fn create_shift(&self, ns: NewShift) -> AppResult<Shift> {
        let mut s = self.state.write().unwrap();
        let id = Uuid::new_v4();
        let shift = Shift {
            id,
            schedule_id: ns.schedule_id,
            starts_at: ns.starts_at,
            ends_at: ns.ends_at,
            period: ns.period,
            assigned_user_id: None,
            created_by: ns.created_by,
            created_at: Utc::now(),
        };
        s.shifts.insert(id, shift.clone());
        Ok(shift)
    }

    async fn list_shifts(
        &self,
        schedule_id: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> AppResult<Vec<Shift>> {
        let s = self.state.read().unwrap();
        let mut out: Vec<_> = s
            .shifts
            .values()
            .filter(|x| x.schedule_id == schedule_id && x.starts_at >= from && x.starts_at < to)
            .cloned()
            .collect();
        out.sort_by_key(|x| x.starts_at);
        Ok(out)
    }

    async fn get_shift(&self, shift_id: Uuid) -> AppResult<Option<Shift>> {
        Ok(self.state.read().unwrap().shifts.get(&shift_id).cloned())
    }

    async fn assign_shift(&self, shift_id: Uuid, assigned_user_id: Option<Uuid>) -> AppResult<()> {
        let mut s = self.state.write().unwrap();
        let Some(shift) = s.shifts.get_mut(&shift_id) else {
            return Err(AppError::NotFound);
        };
        shift.assigned_user_id = assigned_user_id;
        Ok(())
    }

    async fn add_shift_comment(&self, nc: NewShiftComment) -> AppResult<ShiftComment> {
        let mut s = self.state.write().unwrap();
        let c = ShiftComment {
            id: Uuid::new_v4(),
            shift_id: nc.shift_id,
            user_id: nc.user_id,
            body: nc.body,
            created_at: Utc::now(),
        };
        s.comments.entry(nc.shift_id).or_default().push(c.clone());
        Ok(c)
    }

    async fn list_shift_comments(&self, shift_id: Uuid) -> AppResult<Vec<ShiftComment>> {
        Ok(self
            .state
            .read()
            .unwrap()
            .comments
            .get(&shift_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn create_template(&self, nt: NewTemplate) -> AppResult<RotationTemplate> {
        let mut s = self.state.write().unwrap();
        let t = RotationTemplate {
            id: Uuid::new_v4(),
            schedule_id: nt.schedule_id,
            name: nt.name,
            definition: nt.definition,
            created_by: nt.created_by,
            created_at: Utc::now(),
        };
        s.templates.insert(t.id, t.clone());
        Ok(t)
    }

    async fn list_templates(&self, schedule_id: Uuid) -> AppResult<Vec<RotationTemplate>> {
        let s = self.state.read().unwrap();
        let mut out: Vec<_> = s
            .templates
            .values()
            .filter(|x| x.schedule_id == schedule_id)
            .cloned()
            .collect();
        out.sort_by_key(|x| x.created_at);
        out.reverse();
        Ok(out)
    }

    async fn get_template(&self, template_id: Uuid) -> AppResult<Option<RotationTemplate>> {
        Ok(self
            .state
            .read()
            .unwrap()
            .templates
            .get(&template_id)
            .cloned())
    }
}
