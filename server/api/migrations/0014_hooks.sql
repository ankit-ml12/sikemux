create table hooks (
  id text primary key check (id ~ '^[0-9a-f]{32}$'),
  user_id text not null references users (id) on delete cascade,
  host_key text not null references devices (key) on delete cascade,
  token_sha256 bytea not null check (length(token_sha256) = 32),
  created_at timestamptz not null default now()
);

create index hooks_by_host on hooks (host_key);

create table hook_deliveries (
  id bigserial primary key,
  hook_id text not null references hooks (id) on delete cascade,
  host_key text not null,
  received_at timestamptz not null default now(),
  method text not null check (method in ('post', 'put', 'patch')),
  headers jsonb not null,
  query text not null,
  body bytea not null check (length(body) <= 1048576),
  attempts integer not null default 0,
  retry_at timestamptz
);

create index hook_deliveries_by_host on hook_deliveries (host_key, id);
create index hook_deliveries_by_age on hook_deliveries (received_at);
