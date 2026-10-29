alter table stash_items
  add column links jsonb,
  add check (links is null or kind in ('text', 'prompt'));
