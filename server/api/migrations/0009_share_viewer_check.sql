alter table shares add column viewer_check text check (viewer_check ~ '^[0-9a-f]{64}$');
