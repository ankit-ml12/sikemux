create table stash_items (
  id text primary key,
  user_id text not null references users (id) on delete cascade,
  kind text not null check (kind in ('link', 'text', 'image', 'video', 'file')),
  url text,
  text text,
  note text,
  name text,
  mime text,
  size bigint check (size >= 0),
  preview jsonb,
  target jsonb,
  from_device text,
  created_at timestamptz not null default now(),
  taken_at timestamptz,
  check ((kind = 'link') = (url is not null)),
  check ((kind = 'text') = (text is not null)),
  check ((kind in ('image', 'video', 'file')) = (size is not null and name is not null and mime is not null))
);

create index stash_items_by_user on stash_items (user_id, created_at desc, id desc);
create index stash_items_taken on stash_items (taken_at) where taken_at is not null;

alter table events
  drop constraint events_type_check,
  add constraint events_type_check check (
    type in (
      'device.added', 'device.changed', 'device.revoked', 'account.deleted',
      'stash.added', 'stash.changed', 'stash.removed'
    )
  );
