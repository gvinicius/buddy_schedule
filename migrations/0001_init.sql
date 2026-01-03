-- Users
create table if not exists app_user (
  id uuid primary key,
  email text not null unique,
  password_hash text not null,
  is_superadmin boolean not null default false,
  created_at timestamptz not null default now()
);

-- Schedules (a "crowd schedule" for a subject: person/family/pet/etc)
create table if not exists schedule (
  id uuid primary key,
  name text not null,
  subject_type text not null,
  subject_name text not null,
  created_by uuid not null references app_user(id) on delete restrict,
  created_at timestamptz not null default now()
);

-- Schedule membership + per-schedule role
create table if not exists schedule_member (
  schedule_id uuid not null references schedule(id) on delete cascade,
  user_id uuid not null references app_user(id) on delete cascade,
  role text not null, -- 'admin' | 'user'
  created_at timestamptz not null default now(),
  primary key (schedule_id, user_id)
);

-- Rotation templates: JSON definition for week patterns
create table if not exists rotation_template (
  id uuid primary key,
  schedule_id uuid not null references schedule(id) on delete cascade,
  name text not null,
  definition jsonb not null,
  created_by uuid not null references app_user(id) on delete restrict,
  created_at timestamptz not null default now()
);

-- Shifts/time slots
create table if not exists shift (
  id uuid primary key,
  schedule_id uuid not null references schedule(id) on delete cascade,
  starts_at timestamptz not null,
  ends_at timestamptz not null,
  period text not null, -- 'morning'|'afternoon'|'night'|'sleep'
  assigned_user_id uuid null references app_user(id) on delete set null,
  created_by uuid not null references app_user(id) on delete restrict,
  created_at timestamptz not null default now(),
  constraint shift_time_ok check (ends_at > starts_at)
);
create index if not exists idx_shift_schedule_time on shift(schedule_id, starts_at, ends_at);

-- Shift comments / rotation notes
create table if not exists shift_comment (
  id uuid primary key,
  shift_id uuid not null references shift(id) on delete cascade,
  user_id uuid not null references app_user(id) on delete cascade,
  body text not null,
  created_at timestamptz not null default now()
);
create index if not exists idx_shift_comment_shift on shift_comment(shift_id, created_at);
