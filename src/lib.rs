pub mod auth;
pub mod config;
pub mod error;
pub mod models;
pub mod repo;

use crate::{
    auth::{decode_jwt, hash_password, issue_jwt, verify_password, JwtKeys},
    error::{AppError, AppResult},
    models::{Period, ScheduleRole, User},
    repo::{NewSchedule, NewShift, NewShiftComment, NewTemplate, NewUser, Repo},
};
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<dyn Repo>,
    pub jwt: JwtKeys,
    pub cors_origin: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: Uuid,
    pub is_superadmin: bool,
}

impl AuthUser {
    async fn from_headers(state: &AppState, headers: &HeaderMap) -> AppResult<Self> {
        let authz = headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or(AppError::Unauthorized)?;
        let token = authz
            .strip_prefix("Bearer ")
            .ok_or(AppError::Unauthorized)?;
        let claims = decode_jwt(token, &state.jwt)?;
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized)?;
        // Ensure user still exists
        let user = state
            .repo
            .get_user(user_id)
            .await?
            .ok_or(AppError::Unauthorized)?;
        Ok(Self {
            id: user.id,
            is_superadmin: user.is_superadmin,
        })
    }
}

pub fn build_router(state: AppState) -> Router {
    let mut cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    cors = match &state.cors_origin {
        None => cors.allow_origin(tower_http::cors::Any),
        Some(origin) => cors.allow_origin(
            origin
                .parse::<HeaderValue>()
                .expect("CORS_ORIGIN must be a valid header value"),
        ),
    };

    Router::new()
        .route("/healthz", get(healthz))
        .nest(
            "/api",
            Router::new()
                .route("/auth/register", post(register))
                .route("/auth/login", post(login))
                .route("/me", get(me))
                .route("/schedules", get(list_schedules).post(create_schedule))
                .route(
                    "/schedules/:schedule_id/members",
                    get(list_members).post(add_member),
                )
                .route(
                    "/schedules/:schedule_id/members/:user_id/role",
                    post(set_member_role),
                )
                .route(
                    "/schedules/:schedule_id/shifts",
                    get(list_shifts).post(create_shift),
                )
                .route("/shifts/:shift_id/assign", post(assign_shift))
                .route("/shifts/:shift_id/comments", post(add_shift_comment))
                .route(
                    "/schedules/:schedule_id/templates",
                    get(list_templates).post(create_template),
                )
                .route(
                    "/schedules/:schedule_id/templates/:template_id/apply",
                    post(apply_template),
                ),
        )
        .fallback_service(ServeDir::new("web"))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

async fn healthz() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true }))
}

#[derive(Debug, Deserialize)]
struct AuthRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    token: String,
}

async fn register(
    State(state): State<AppState>,
    Json(req): Json<AuthRequest>,
) -> AppResult<Json<AuthResponse>> {
    let email = req.email.trim().to_lowercase();
    if email.is_empty() || req.password.len() < 8 {
        return Err(AppError::BadRequest(
            "email must be set and password must be >= 8 chars".to_string(),
        ));
    }

    let is_superadmin = state.repo.count_users().await? == 0;
    let password_hash = hash_password(&req.password)?;
    let user = state
        .repo
        .create_user(NewUser {
            email,
            password_hash,
            is_superadmin,
        })
        .await?;

    let token = issue_jwt(user.id, user.is_superadmin, &state.jwt)?;
    Ok(Json(AuthResponse { token }))
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<AuthRequest>,
) -> AppResult<Json<AuthResponse>> {
    let email = req.email.trim().to_lowercase();
    let Some((user, password_hash)) = state.repo.find_user_by_email(&email).await? else {
        return Err(AppError::Unauthorized);
    };
    if !verify_password(&req.password, &password_hash)? {
        return Err(AppError::Unauthorized);
    }
    let token = issue_jwt(user.id, user.is_superadmin, &state.jwt)?;
    Ok(Json(AuthResponse { token }))
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    let user = state
        .repo
        .get_user(au.id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(Json(user))
}

async fn require_member_or_superadmin(
    state: &AppState,
    au: &AuthUser,
    schedule_id: Uuid,
) -> AppResult<ScheduleRole> {
    if au.is_superadmin {
        return Ok(ScheduleRole::Admin);
    }
    state
        .repo
        .get_schedule_role(schedule_id, au.id)
        .await?
        .ok_or(AppError::Forbidden)
}

async fn require_admin_or_superadmin(
    state: &AppState,
    au: &AuthUser,
    schedule_id: Uuid,
) -> AppResult<()> {
    if au.is_superadmin {
        return Ok(());
    }
    let role = state
        .repo
        .get_schedule_role(schedule_id, au.id)
        .await?
        .ok_or(AppError::Forbidden)?;
    if role != ScheduleRole::Admin {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

async fn list_schedules(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    let schedules = state.repo.list_schedules_for_user(au.id).await?;
    Ok(Json(schedules))
}

#[derive(Debug, Deserialize)]
struct CreateScheduleRequest {
    name: String,
    subject_type: String,
    subject_name: String,
}

async fn create_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateScheduleRequest>,
) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    let schedule = state
        .repo
        .create_schedule(NewSchedule {
            name: req.name.trim().to_string(),
            subject_type: req.subject_type.trim().to_string(),
            subject_name: req.subject_name.trim().to_string(),
            created_by: au.id,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(schedule)))
}

#[derive(Debug, Deserialize)]
struct AddMemberRequest {
    email: String,
    role: ScheduleRole,
}

#[derive(Debug, Serialize)]
struct MemberWithRole {
    user: User,
    role: ScheduleRole,
}

async fn list_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(schedule_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    require_member_or_superadmin(&state, &au, schedule_id).await?;
    let members = state.repo.list_schedule_members(schedule_id).await?;
    let response: Vec<MemberWithRole> = members
        .into_iter()
        .map(|(user, role)| MemberWithRole { user, role })
        .collect();
    Ok(Json(response))
}

async fn add_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(schedule_id): Path<Uuid>,
    Json(req): Json<AddMemberRequest>,
) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    require_admin_or_superadmin(&state, &au, schedule_id).await?;

    let email = req.email.trim().to_lowercase();
    let Some((user, _)) = state.repo.find_user_by_email(&email).await? else {
        return Err(AppError::BadRequest("user email not found".to_string()));
    };
    state
        .repo
        .add_member(schedule_id, user.id, req.role)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct SetRoleRequest {
    role: ScheduleRole,
}

async fn set_member_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((schedule_id, user_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<SetRoleRequest>,
) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    require_admin_or_superadmin(&state, &au, schedule_id).await?;
    state
        .repo
        .set_member_role(schedule_id, user_id, req.role)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct CreateShiftRequest {
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    period: Period,
}

async fn create_shift(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(schedule_id): Path<Uuid>,
    Json(req): Json<CreateShiftRequest>,
) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    require_admin_or_superadmin(&state, &au, schedule_id).await?;

    let shift = state
        .repo
        .create_shift(NewShift {
            schedule_id,
            starts_at: req.starts_at,
            ends_at: req.ends_at,
            period: req.period,
            created_by: au.id,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(shift)))
}

#[derive(Debug, Deserialize)]
struct ListShiftsQuery {
    from: String,
    to: String,
}

async fn list_shifts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(schedule_id): Path<Uuid>,
    Query(q): Query<ListShiftsQuery>,
) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    require_member_or_superadmin(&state, &au, schedule_id).await?;

    let from = DateTime::parse_from_rfc3339(&q.from)
        .map_err(|_| AppError::BadRequest("invalid from (RFC3339 required)".to_string()))?
        .with_timezone(&Utc);
    let to = DateTime::parse_from_rfc3339(&q.to)
        .map_err(|_| AppError::BadRequest("invalid to (RFC3339 required)".to_string()))?
        .with_timezone(&Utc);

    let shifts = state.repo.list_shifts(schedule_id, from, to).await?;
    Ok(Json(shifts))
}

#[derive(Debug, Deserialize)]
struct AssignShiftRequest {
    assigned_user_id: Option<Uuid>,
}

async fn assign_shift(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(shift_id): Path<Uuid>,
    Json(req): Json<AssignShiftRequest>,
) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    let shift = state
        .repo
        .get_shift(shift_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let role = require_member_or_superadmin(&state, &au, shift.schedule_id).await?;
    let target = req.assigned_user_id.unwrap_or(au.id);

    // Only admins can assign other users.
    if target != au.id && !(au.is_superadmin || role == ScheduleRole::Admin) {
        return Err(AppError::Forbidden);
    }

    state.repo.assign_shift(shift_id, Some(target)).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct AddCommentRequest {
    body: String,
}

async fn add_shift_comment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(shift_id): Path<Uuid>,
    Json(req): Json<AddCommentRequest>,
) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    let shift = state
        .repo
        .get_shift(shift_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let role = require_member_or_superadmin(&state, &au, shift.schedule_id).await?;

    // Only assigned user or admins can comment.
    if !au.is_superadmin && role != ScheduleRole::Admin && shift.assigned_user_id != Some(au.id) {
        return Err(AppError::Forbidden);
    }
    if req.body.trim().is_empty() {
        return Err(AppError::BadRequest("comment body is required".to_string()));
    }
    let c = state
        .repo
        .add_shift_comment(NewShiftComment {
            shift_id,
            user_id: au.id,
            body: req.body.trim().to_string(),
        })
        .await?;
    Ok((StatusCode::CREATED, Json(c)))
}

#[derive(Debug, Deserialize)]
struct CreateTemplateRequest {
    name: String,
    definition: serde_json::Value,
}

async fn create_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(schedule_id): Path<Uuid>,
    Json(req): Json<CreateTemplateRequest>,
) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    require_admin_or_superadmin(&state, &au, schedule_id).await?;
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    let t = state
        .repo
        .create_template(NewTemplate {
            schedule_id,
            name: req.name.trim().to_string(),
            definition: req.definition,
            created_by: au.id,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(t)))
}

async fn list_templates(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(schedule_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    require_member_or_superadmin(&state, &au, schedule_id).await?;
    Ok(Json(state.repo.list_templates(schedule_id).await?))
}

#[derive(Debug, Deserialize)]
struct ApplyTemplateRequest {
    week_start: String, // YYYY-MM-DD (UTC Monday recommended)
}

#[derive(Debug, Deserialize)]
struct TemplateSlot {
    dow: i64,       // 0=Mon..6=Sun
    period: Period, // morning/afternoon/night/sleep
    start: String,  // HH:MM
    end: String,    // HH:MM
}

#[derive(Debug, Deserialize)]
struct TemplateDef {
    slots: Vec<TemplateSlot>,
}

async fn apply_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((schedule_id, template_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<ApplyTemplateRequest>,
) -> AppResult<impl IntoResponse> {
    let au = AuthUser::from_headers(&state, &headers).await?;
    require_admin_or_superadmin(&state, &au, schedule_id).await?;

    let template = state
        .repo
        .get_template(template_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if template.schedule_id != schedule_id {
        return Err(AppError::Forbidden);
    }

    let week_start = NaiveDate::parse_from_str(&req.week_start, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("week_start must be YYYY-MM-DD".to_string()))?;

    let def: TemplateDef = serde_json::from_value(template.definition)
        .map_err(|_| AppError::BadRequest("invalid template definition".to_string()))?;

    let mut created = Vec::with_capacity(def.slots.len());
    for slot in def.slots {
        if !(0..=6).contains(&slot.dow) {
            return Err(AppError::BadRequest("slot.dow must be 0..6".to_string()));
        }
        let start_t = NaiveTime::parse_from_str(&slot.start, "%H:%M")
            .map_err(|_| AppError::BadRequest("slot.start must be HH:MM".to_string()))?;
        let end_t = NaiveTime::parse_from_str(&slot.end, "%H:%M")
            .map_err(|_| AppError::BadRequest("slot.end must be HH:MM".to_string()))?;

        let day = week_start
            .checked_add_signed(chrono::Duration::days(slot.dow))
            .ok_or_else(|| AppError::BadRequest("invalid date".to_string()))?;

        let start_naive = day.and_time(start_t);
        let mut end_naive = day.and_time(end_t);
        if end_naive <= start_naive {
            end_naive += chrono::Duration::days(1);
        }

        let starts_at = DateTime::<Utc>::from_naive_utc_and_offset(start_naive, Utc);
        let ends_at = DateTime::<Utc>::from_naive_utc_and_offset(end_naive, Utc);

        let shift = state
            .repo
            .create_shift(NewShift {
                schedule_id,
                starts_at,
                ends_at,
                period: slot.period,
                created_by: au.id,
            })
            .await?;
        created.push(shift);
    }

    Ok((StatusCode::CREATED, Json(created)))
}

#[cfg(test)]
mod api_tests {
    use super::*;
    use crate::repo::MemRepo;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn router() -> Router {
        build_router(AppState {
            repo: Arc::new(MemRepo::new()),
            jwt: JwtKeys::new("test-secret"),
            cors_origin: None,
        })
    }

    #[tokio::test]
    async fn register_login_and_create_schedule() {
        let app = router();

        // Register
        let res = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/auth/register")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        r#"{"email":"a@example.com","password":"password1"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let token = v.get("token").unwrap().as_str().unwrap().to_string();

        // Create schedule
        let res = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/schedules")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {token}"))
                    .body(axum::body::Body::from(
                        r#"{"name":"Care","subject_type":"pet","subject_name":"Puppy"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }
}
