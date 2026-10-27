alter table stash_items
  add column prompt_id text references stash_items (id) on delete cascade,
  add column attachments_missing integer not null default 0
    check (attachments_missing between 0 and 20),
  drop constraint stash_items_kind_check,
  add constraint stash_items_kind_check
    check (kind in ('link', 'text', 'image', 'video', 'file', 'prompt')),
  drop constraint stash_items_check1,
  add constraint stash_items_check1 check ((kind in ('text', 'prompt')) = (text is not null)),
  add check (prompt_id is null or kind in ('image', 'video', 'file')),
  add check (attachments_missing = 0 or kind = 'prompt');

create index stash_items_by_prompt on stash_items (prompt_id) where prompt_id is not null;
